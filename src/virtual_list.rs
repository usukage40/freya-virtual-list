//! A `VirtualList` component for Freya.
//!
//! Renders only the items currently inside the viewport (plus an overscan
//! margin), supports **variable item heights** by measuring each rendered item
//! at runtime, and keeps the scroll position stable when item heights above
//! the viewport are corrected after measurement.
//!
//! ## Example
//!
//! ```rust, no_run
//! # use freya::prelude::*;
//! use freya_virtual_list::VirtualList;
//!
//! fn app() -> impl IntoElement {
//!     let items = use_state(|| vec![
//!         "Hello".to_string(),
//!         "World".to_string(),
//!         "Variable height items".to_string(),
//!     ]);
//!
//!     let items_clone = items.read().clone();
//!     VirtualList::new(items_clone, move |index, item, _measured_h| {
//!         rect()
//!             .key(index)
//!             .height(Size::px(50.))
//!             .padding(4.)
//!             .child(format!("#{index}: {item}"))
//!             .into()
//!     })
//!     .default_item_height(50.)
//!     .item_gap(4.)
//! }
//! ```

use std::time::Duration;

use freya::{animation::*, prelude::*, sdk::use_timeout};

/// Number of items grouped together for incremental prefix-sum updates.
///
/// Chunked sums keep per-measurement updates O(1) instead of recomputing the
/// whole prefix-sum array (O(n)) every time a single item height is corrected.
const CHUNK_SIZE: usize = 64;

/// Height bookkeeping: per-item measured heights plus per-chunk prefix sums.
#[derive(Debug, Clone, PartialEq)]
struct HeightData {
    /// `Some(h)` once an item has been rendered and measured, `None` otherwise.
    measured: Vec<Option<f32>>,
    /// Sum of effective heights (measured or default) per chunk of `CHUNK_SIZE`.
    chunk_sums: Vec<f32>,
    /// Fallback height used for items that have not been measured yet.
    default_h: f32,
    /// Vertical gap between items.
    gap: f32,
}

impl HeightData {
    fn new(len: usize, default_h: f32, gap: f32) -> Self {
        let chunk_sums = (0..len.div_ceil(CHUNK_SIZE))
            .map(|c| {
                let start = c * CHUNK_SIZE;
                let end = (start + CHUNK_SIZE).min(len);
                default_h * (end - start) as f32
            })
            .collect();
        Self {
            measured: vec![None; len],
            chunk_sums,
            default_h,
            gap,
        }
    }

    fn len(&self) -> usize {
        self.measured.len()
    }

    fn height(&self, i: usize) -> f32 {
        self.measured[i].unwrap_or(self.default_h)
    }

    /// Update a measured height in place. Returns `true` if the value changed.
    fn set_measured(&mut self, i: usize, h: f32) -> bool {
        let old = self.height(i);
        if (old - h).abs() < f32::EPSILON {
            return false;
        }
        self.measured[i] = Some(h);
        self.chunk_sums[i / CHUNK_SIZE] += h - old;
        true
    }

    /// Cumulative pixel offset of the top of item `i` (0-indexed).
    fn offset_of(&self, i: usize) -> f32 {
        let i = i.min(self.measured.len());
        let chunk = i / CHUNK_SIZE;
        let mut acc = 0.0;
        for c in 0..chunk {
            acc += self.chunk_sums[c];
        }
        let base = chunk * CHUNK_SIZE;
        for j in base..i {
            acc += self.height(j);
        }
        acc + i as f32 * self.gap
    }

    /// Total height of all items (no trailing gap).
    fn total_height(&self) -> f32 {
        let n = self.measured.len();
        if n == 0 {
            return 0.0;
        }
        let sum: f32 = self.chunk_sums.iter().sum();
        sum + (n - 1) as f32 * self.gap
    }

    /// Returns `(index, offset_into_item)` of the item covering pixel `y`.
    fn item_at(&self, y: f32) -> (usize, f32) {
        let n = self.measured.len();
        if n == 0 {
            return (0, 0.0);
        }
        let mut acc = 0.0;
        for c in 0..self.chunk_sums.len() {
            let base = c * CHUNK_SIZE;
            let end = (base + CHUNK_SIZE).min(n);
            let count = end - base;
            let chunk_h = self.chunk_sums[c] + count.saturating_sub(1) as f32 * self.gap;
            if acc + chunk_h >= y {
                for j in base..end {
                    let h = self.height(j);
                    if acc + h >= y {
                        return (j, (y - acc).clamp(0.0, h));
                    }
                    acc += h + self.gap;
                }
                return (end - 1, 0.0);
            }
            acc += chunk_h + self.gap;
        }
        let last = n - 1;
        (last, 0.0)
    }
}

/// A virtualized, variable-height list.
pub struct VirtualList<T, B: Fn(usize, &T, Option<f32>) -> Element + 'static> {
    items: Vec<T>,
    default_item_height: f32,
    item_gap: f32,
    overscan: usize,
    render_item: B,
    scrollbar_hide_delay: Duration,
    layout: LayoutData,
}

impl<T: PartialEq, B: Fn(usize, &T, Option<f32>) -> Element> PartialEq for VirtualList<T, B> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
            && self.default_item_height == other.default_item_height
            && self.item_gap == other.item_gap
            && self.overscan == other.overscan
            && self.scrollbar_hide_delay == other.scrollbar_hide_delay
            && self.layout == other.layout
    }
}

impl<T, B: Fn(usize, &T, Option<f32>) -> Element + 'static> VirtualList<T, B> {
    pub fn new(items: Vec<T>, render_item: B) -> Self {
        Self {
            items,
            default_item_height: 80.0,
            item_gap: 0.0,
            overscan: 3,
            render_item,
            scrollbar_hide_delay: Duration::from_millis(800),
            layout: LayoutData::default(),
        }
    }

    pub fn default_item_height(mut self, h: f32) -> Self {
        self.default_item_height = h;
        self
    }

    pub fn item_gap(mut self, gap: f32) -> Self {
        self.item_gap = gap;
        self
    }

    pub fn overscan(mut self, n: usize) -> Self {
        self.overscan = n;
        self
    }

    /// How long the scrollbar stays visible after the last scroll/drag/hover.
    ///
    /// Defaults to `800ms`.
    pub fn scrollbar_hide_delay(mut self, duration: Duration) -> Self {
        self.scrollbar_hide_delay = duration;
        self
    }
}

impl<T: PartialEq + 'static, B: Fn(usize, &T, Option<f32>) -> Element + 'static> Component
    for VirtualList<T, B>
{
    fn render(&self) -> impl IntoElement {
        let item_count = self.items.len();
        let default_h = self.default_item_height;
        let gap = self.item_gap;
        let overscan = self.overscan;

        // All hooks must be called unconditionally and in a stable order,
        // BEFORE any early return, otherwise Freya panics with HOOKS_ERROR.
        let mut heights = use_state(|| HeightData::new(item_count, default_h, gap));
        // Scroll anchor: (item index at the top of the viewport, pixel offset into it).
        // Anchoring the scroll to an item (instead of raw pixels) keeps the content
        // under the viewport stable when heights above the viewport are corrected.
        let mut anchor = use_state(|| (0usize, 0.0f32));
        let mut viewport_h = use_state(|| 0.0f32);
        let mut size = use_state(Area::default);
        // Scrollbar drag: grab offset within the thumb, if dragging.
        let mut drag = use_state(|| None::<f32>);
        // Auto-hide the scrollbar after a period without scrolling/hovering.
        let hide_delay = self.scrollbar_hide_delay;
        let mut scrollbar_timeout = use_timeout(move || hide_delay);
        // Scrollbar fade animation: 1.0 (visible) -> 0.0 (hidden).
        // `on_creation` defaults to `Nothing`, so it only plays when we
        // explicitly call `start()` from the side effect below.
        let mut scrollbar_fade =
            use_animation(|_| AnimNum::new(1.0, 0.0).time(200).ease(Ease::Out));

        // Rebuild height data when the item count changes.
        // Note: the callback is stored once at mount, so it must read the new
        // value from its `&deps` argument instead of a captured stale copy.
        use_side_effect_with_deps(&item_count, move |n| {
            *heights.write() = HeightData::new(*n, default_h, gap);
            *anchor.write() = (0, 0.0);
            *drag.write() = None;
        });

        // Keep the stored anchor valid when the content height or viewport changes.
        // Re-runs whenever any of the states it reads change.
        use_side_effect(move || {
            let vh = *viewport_h.read();
            let hd = heights.read();
            let max_scroll = (hd.total_height() - vh).max(0.0);
            let (ai, ao) = *anchor.read();
            let sy = hd.offset_of(ai) + ao;
            if sy > max_scroll {
                let (ni, _) = hd.item_at(max_scroll);
                let off = max_scroll - hd.offset_of(ni);
                *anchor.write() = (ni, off.max(0.0));
            }
        });

        if item_count == 0 {
            return rect().into_element();
        }

        // Safe effective length: guards against a one-frame mismatch between the
        // new `item_count` and the height data before the side effect above runs.
        let n = item_count.min(heights.read().len());

        let vh = *viewport_h.read();
        let (ai, ao) = *anchor.read();

        let scroll_y = heights.read().offset_of(ai) + ao;
        let total = heights.read().total_height();
        let max_scroll = (total - vh).max(0.0);
        // Clamp the derived scroll so content that shrank below the viewport
        // does not leave a blank area.
        let sy = scroll_y.min(max_scroll);

        // Hide the scrollbar once the inactivity timeout elapses and only when
        // there is actually content overflowing the viewport.
        let scrollbar_visible = !scrollbar_timeout.elapsed() && total > vh;

        // Drive the fade: restore full opacity immediately while the scrollbar
        // should stay visible, otherwise start fading it out. `start()` is only
        // called once the animation has run at least once (i.e. the scrollbar
        // was actually shown), so a never-shown scrollbar does not play.
        use_side_effect_with_deps(&scrollbar_visible, move |&visible| {
            if visible {
                scrollbar_fade.reset();
            } else if *scrollbar_fade.has_run_yet().read() {
                scrollbar_fade.start();
            }
        });

        // Keep the scrollbar mounted while it is fading out, so the transition
        // is actually visible; unmount it once the opacity reaches 0.
        let sb_opacity = scrollbar_fade.get().value();
        let sb_showing = scrollbar_visible || sb_opacity > 0.0;

        let (start_item, _) = heights.read().item_at(sy);
        let render_start = start_item.saturating_sub(overscan);

        let viewport_bottom = sy + vh;
        let mut render_end = render_start;
        let mut acc = heights.read().offset_of(render_start);
        while render_end < n && acc < viewport_bottom {
            acc += heights.read().height(render_end) + gap;
            render_end += 1;
        }
        render_end = (render_end + overscan).min(n);

        let before_height = heights.read().offset_of(render_start);
        let after_start = if render_end < n {
            heights.read().offset_of(render_end)
        } else {
            total
        };
        let after_height = total - after_start;

        // Scrollbar geometry.
        let sb_ratio = if total > 0.0 && vh > 0.0 {
            (vh / total).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let sb_thumb_h = (vh * sb_ratio).max(20.0);
        let sb_thumb_y = if max_scroll > 0.0 && vh > sb_thumb_h {
            (sy / max_scroll) * (vh - sb_thumb_h)
        } else {
            0.0
        };

        let on_wheel = {
            move |e: Event<WheelEventData>| {
                let vh = *viewport_h.read();
                let (ai, ao) = *anchor.read();
                let cur = heights.read().offset_of(ai) + ao;
                let max_scroll = (heights.read().total_height() - vh).max(0.0);
                let new_y = (cur - e.delta_y as f32).clamp(0.0, max_scroll);
                anchor.set(heights.read().item_at(new_y));
                scrollbar_timeout.reset();
            }
        };

        // Scrollbar: start drag / jump to clicked position.
        let on_scrollbar_down = {
            move |e: Event<PointerEventData>| {
                if !e.data().is_primary() {
                    return;
                }
                let loc = e.global_location();
                let top = size.read().min_y() as f64;
                let track_h = *viewport_h.read();
                let total = heights.read().total_height();
                let max_scroll = (total - track_h).max(0.0);
                let rel_y = (loc.y - top).clamp(0.0, track_h as f64) as f32;
                let thumb_h = ((track_h / total.max(1.0)).clamp(0.0, 1.0) * track_h).max(20.0);
                let (ai, ao) = *anchor.read();
                let cur = heights.read().offset_of(ai) + ao;
                let thumb_y = if max_scroll > 0.0 && track_h > thumb_h {
                    (cur / max_scroll) * (track_h - thumb_h)
                } else {
                    0.0
                };

                if rel_y >= thumb_y && rel_y <= thumb_y + thumb_h {
                    // Grabbed the thumb: keep the grab offset stable and don't jump.
                    drag.set(Some(rel_y - thumb_y));
                } else {
                    // Clicked the track: jump to the clicked position and start dragging.
                    drag.set(Some(thumb_h / 2.0));
                    let frac = rel_y / track_h.max(1.0);
                    let target = (frac * max_scroll).clamp(0.0, max_scroll);
                    anchor.set(heights.read().item_at(target));
                }
                scrollbar_timeout.reset();
                e.prevent_default();
            }
        };

        let on_global_pointer_move = {
            move |e: Event<PointerEventData>| {
                if let Some(grab) = *drag.peek() {
                    let loc = e.global_location();
                    let top = size.read().min_y() as f64;
                    let track_h = *viewport_h.read();
                    let total = heights.read().total_height();
                    let max_scroll = (total - track_h).max(0.0);
                    let thumb_h = ((track_h / total.max(1.0)).clamp(0.0, 1.0) * track_h).max(20.0);
                    let thumb_y = (loc.y - top) as f32 - grab;
                    let frac = if track_h > thumb_h {
                        (thumb_y / (track_h - thumb_h)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let target = (frac * max_scroll).clamp(0.0, max_scroll);
                    anchor.set(heights.read().item_at(target));
                    scrollbar_timeout.reset();
                    e.prevent_default();
                }
            }
        };

        let on_capture_global_pointer_press = move |_: Event<PointerEventData>| {
            drag.set(None);
        };

        // Keep the scrollbar visible while the cursor is over the list.
        let on_mouse_move = move |_: Event<MouseEventData>| {
            scrollbar_timeout.reset();
        };

        let on_outer_sized = move |e: Event<SizedEventData>| {
            size.set(e.area);
            viewport_h.set(e.area.height());
        };

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .on_sized(on_outer_sized)
            .on_wheel(on_wheel)
            .on_mouse_move(on_mouse_move)
            .on_global_pointer_move(on_global_pointer_move)
            .on_capture_global_pointer_press(on_capture_global_pointer_press)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(total))
                    .offset_y(-sy)
                    .child(rect().height(Size::px(before_height)).width(Size::fill()))
                    .child(rect().width(Size::fill()).spacing(gap).children(
                        self.items[render_start..render_end].iter().enumerate().map(
                            |(idx, item)| {
                                let i = render_start + idx;
                                let measured_h = heights.read().measured[i];
                                rect()
                                    .key(i)
                                    .width(Size::fill())
                                    .on_sized({
                                        let mut heights = heights;
                                        move |e: Event<SizedEventData>| {
                                            let h = e.area.height();
                                            if heights.peek().measured[i] != Some(h) {
                                                heights.write().set_measured(i, h);
                                            }
                                        }
                                    })
                                    .child((self.render_item)(i, item, measured_h))
                                    .into()
                            },
                        ),
                    ))
                    .child(rect().height(Size::px(after_height)).width(Size::fill())),
            )
            .maybe_child(
                sb_showing.then_some(
                    rect()
                        .width(Size::px(8.0))
                        .height(Size::px(vh))
                        .position(Position::new_absolute().top(0.).right(6.))
                        .layer(999)
                        .background((60, 60, 65))
                        .corner_radius(4.)
                        .opacity(sb_opacity)
                        .on_pointer_down(on_scrollbar_down)
                        .child(
                            rect()
                                .width(Size::fill())
                                .height(Size::px(sb_thumb_h))
                                .position(Position::new_absolute().top(sb_thumb_y).left(0.))
                                .background((120, 120, 130))
                                .corner_radius(4.),
                        ),
                ),
            )
            .into()
    }
}

impl<T: PartialEq + 'static, B: Fn(usize, &T, Option<f32>) -> Element + 'static> LayoutExt
    for VirtualList<T, B>
{
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl<T: PartialEq + 'static, B: Fn(usize, &T, Option<f32>) -> Element + 'static> ContainerSizeExt
    for VirtualList<T, B>
{
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hd(len: usize) -> HeightData {
        HeightData::new(len, 40.0, 4.0)
    }

    #[test]
    fn defaults_to_default_height() {
        let data = hd(5);
        assert_eq!(data.height(0), 40.0);
        assert_eq!(data.total_height(), 40.0 * 5.0 + 4.0 * 4.0);
    }

    #[test]
    fn offsets_include_gaps() {
        let mut data = hd(3);
        data.set_measured(0, 50.0);
        data.set_measured(2, 60.0);
        // offset(0)=0, offset(1)=50+4=54, offset(2)=50+4+40+4=98
        assert_eq!(data.offset_of(0), 0.0);
        assert_eq!(data.offset_of(1), 54.0);
        assert_eq!(data.offset_of(2), 98.0);
        assert_eq!(data.total_height(), 50.0 + 40.0 + 60.0 + 4.0 * 2.0);
    }

    #[test]
    fn measured_updates_are_idempotent() {
        let mut data = hd(2);
        assert!(data.set_measured(0, 100.0));
        assert!(!data.set_measured(0, 100.0));
        assert!(data.set_measured(0, 90.0));
        assert_eq!(data.height(0), 90.0);
        assert_eq!(data.total_height(), 90.0 + 40.0 + 4.0);
    }

    #[test]
    fn item_at_maps_pixels_to_items() {
        let mut data = hd(3);
        data.set_measured(0, 50.0);
        // item 0 spans [0,50), item 1 spans [54,94), item 2 spans [98,138)
        assert_eq!(data.item_at(0.0), (0, 0.0));
        assert_eq!(data.item_at(49.0), (0, 49.0));
        assert_eq!(data.item_at(54.0), (1, 0.0));
        assert_eq!(data.item_at(60.0), (1, 6.0));
        assert_eq!(data.item_at(98.0), (2, 0.0));
        // past the end clamps to the last item
        assert_eq!(data.item_at(500.0), (2, 0.0));
    }

    #[test]
    fn chunks_scale_across_boundaries() {
        // Use a list larger than CHUNK_SIZE to exercise chunked sums.
        let len = CHUNK_SIZE * 2 + 7;
        let mut data = hd(len);
        for i in 0..len {
            data.set_measured(i, 20.0 + (i % 5) as f32);
        }
        let expected_total: f32 =
            (0..len).map(|i| 20.0 + (i % 5) as f32).sum::<f32>() + (len - 1) as f32 * 4.0;
        assert!((data.total_height() - expected_total).abs() < 0.001);

        // offset_of matches a naive accumulation.
        let mut acc = 0.0;
        for i in 0..len {
            assert!((data.offset_of(i) - acc).abs() < 0.001);
            acc += data.height(i) + 4.0;
        }
    }

    #[test]
    fn resize_preserves_empty_state() {
        let data = hd(0);
        assert_eq!(data.total_height(), 0.0);
        assert_eq!(data.item_at(0.0), (0, 0.0));
    }
}
