use std::time::Duration;

use freya::prelude::*;
use freya_testing::prelude::*;
use freya_virtual_list::VirtualList;

/// Renders `count` items, each exactly `height` pixels tall, labeled `"Item {i}"`.
fn test_app(count: usize, height: f32) -> impl Fn() -> Element {
    move || {
        let items = (0..count).map(|i| format!("Item {i}")).collect::<Vec<_>>();
        let height = height;
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                VirtualList::new(items, move |i, text, _measured_h| {
                    label()
                        .key(i)
                        .height(Size::px(height))
                        .text(text.clone())
                        .into()
                })
                .default_item_height(height)
                .item_gap(0.),
            )
            .into()
    }
}

/// Collect the text of every rendered label, in order.
fn rendered_labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

/// The vertical scrollbar track is the only `rect` exactly `8px` wide.
fn scrollbar_visible(test: &TestingRunner) -> bool {
    test.find(|node, element| {
        Rect::try_downcast(element)
            .filter(|_| (node.layout().area.width() - 8.0).abs() < 0.5)
            .map(|_| ())
    })
    .is_some()
}

#[test]
fn renders_only_visible_items() {
    let mut test = launch_test(test_app(1000, 50.));
    test.sync_and_update();

    let labels = rendered_labels(&test);
    // Default test viewport is 250x250, so only ~5 items should be rendered,
    // not all 1000. Virtualization works.
    assert!(!labels.is_empty());
    assert!(labels.len() < 20);
    assert_eq!(labels[0], "Item 0");
}

#[test]
fn scroll_changes_visible_window() {
    let mut test = launch_test(test_app(1000, 50.));
    test.sync_and_update();

    let labels_before = rendered_labels(&test);
    assert_eq!(labels_before[0], "Item 0");

    // Scroll down 200px => 4 items.
    test.scroll((5., 5.), (0., -200.));
    test.sync_and_update();

    let labels_after = rendered_labels(&test);
    // Overscan keeps the very top item, but the window must have shifted:
    // items far below the old window must now be rendered.
    assert!(labels_after.contains(&"Item 7".to_string()));
    assert_ne!(labels_after, labels_before);
}

#[test]
fn empty_list_does_not_panic() {
    let mut test = launch_test(test_app(0, 50.));
    test.sync_and_update();
    test.scroll((5., 5.), (0., -100.));
    test.sync_and_update();
}

#[test]
fn variable_heights_render_visible_items() {
    // Items alternate between 30px and 120px tall.
    let mut test = launch_test(|| -> Element {
        let items = (0..1000).map(|i| format!("Item {i}")).collect::<Vec<_>>();
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                VirtualList::new(items, move |i, text, _measured_h| {
                    let h = if i % 2 == 0 { 30. } else { 120. };
                    label().key(i).height(Size::px(h)).text(text.clone()).into()
                })
                .default_item_height(75.)
                .item_gap(0.),
            )
            .into()
    });
    test.sync_and_update();

    // Virtualization must hold regardless of variable heights: far-away items
    // are never mounted.
    let labels = rendered_labels(&test);
    assert!(!labels.is_empty());
    assert!(labels.len() < 20);
    assert!(!labels.contains(&"Item 100".to_string()));
}

#[test]
fn scrollbar_auto_hides_when_idle() {
    let mut test = launch_test(|| -> Element {
        let items = (0..1000).map(|i| format!("Item {i}")).collect::<Vec<_>>();
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                VirtualList::new(items, move |i, text, _measured_h| {
                    label()
                        .key(i)
                        .height(Size::px(50.))
                        .text(text.clone())
                        .into()
                })
                .default_item_height(50.)
                .scrollbar_hide_delay(Duration::from_millis(100)),
            )
            .into()
    });
    test.sync_and_update();

    // Shown on first render.
    assert!(scrollbar_visible(&test));

    // Once the inactivity timeout elapses the scrollbar does not disappear
    // instantly: it stays mounted while it fades out over ~200ms.
    test.poll(Duration::from_millis(10), Duration::from_millis(150));
    assert!(scrollbar_visible(&test)); // still fading out

    // ...and is fully unmounted once the fade-out finishes.
    test.poll(Duration::from_millis(10), Duration::from_millis(400));
    assert!(!scrollbar_visible(&test));

    // Scrolling brings it back.
    test.scroll((5., 5.), (0., -500.));
    test.sync_and_update();
    assert!(scrollbar_visible(&test));

    // And it hides again once the user stops scrolling.
    test.poll(Duration::from_millis(10), Duration::from_millis(500));
    assert!(!scrollbar_visible(&test));
}

#[test]
fn scrollbar_drag_scrolls() {
    let mut test = launch_test(test_app(1000, 50.));
    test.sync_and_update();

    // Default test viewport is 500x500. The scrollbar track sits at the right
    // edge (width 8, right offset 6) => x ≈ 486..494, full viewport height.
    test.scroll((5., 5.), (0., -500.));
    test.sync_and_update();
    assert!(rendered_labels(&test).contains(&"Item 6".to_string()));

    // Drag the thumb from the top (y≈100) down to y≈400.
    test.move_cursor((490., 100.));
    test.sync_and_update();
    test.press_cursor((490., 100.));
    test.sync_and_update();
    test.move_cursor((490., 400.));
    test.sync_and_update();
    test.release_cursor((490., 400.));
    test.sync_and_update();

    let labels = rendered_labels(&test);
    // Scrolling down must show items far beyond the original window.
    let first: usize = labels[0].strip_prefix("Item ").unwrap().parse().unwrap();
    assert!(
        first > 500,
        "expected to scroll far down, got first item: {first}"
    );
}
