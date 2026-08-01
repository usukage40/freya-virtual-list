use std::{cell::RefCell, rc::Rc, time::Duration};

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

/// A bottom-sticking list whose items live in shared state, so tests can
/// append items and bump a global state to trigger a re-render.
fn stick_list_app(
    count: usize,
    height: f32,
) -> (Rc<RefCell<Vec<String>>>, State<u32>, impl Fn() -> Element) {
    let items = Rc::new(RefCell::new(
        (0..count).map(|i| format!("Item {i}")).collect::<Vec<_>>(),
    ));
    let bump = State::create_global(0u32);
    let app_items = items.clone();
    let app_bump = bump;
    let app = move || -> Element {
        let _ = *app_bump.read(); // re-render when the test bumps
        let list = app_items.borrow().clone();
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                VirtualList::new(list, move |i, text, _measured_h| {
                    label()
                        .key(i)
                        .height(Size::px(height))
                        .text(text.clone())
                        .into()
                })
                .default_item_height(height)
                .stick_to_bottom(true),
            )
            .into()
    };
    (items, bump, app)
}

#[test]
fn stick_to_bottom_starts_at_bottom() {
    let (_, _, app) = stick_list_app(1000, 50.);
    let mut test = launch_test(app);
    test.sync_and_update();

    let labels = rendered_labels(&test);
    // The last item is visible and everything rendered is near the end
    // (the first rendered label is the overscan margin above the viewport).
    assert!(labels.contains(&"Item 999".to_string()));
    let first: usize = labels[0].strip_prefix("Item ").unwrap().parse().unwrap();
    assert!(
        first >= 980,
        "expected to start at the bottom, got first item: {first}"
    );
}

#[test]
fn follows_appended_items() {
    let (items, mut bump, app) = stick_list_app(1000, 50.);
    let mut test = launch_test(app);
    test.sync_and_update();
    assert!(rendered_labels(&test).contains(&"Item 999".to_string()));

    // Appending an item keeps the viewport glued to the bottom.
    items.borrow_mut().push("Item 1000".to_string());
    *bump.write() += 1;
    test.sync_and_update();
    test.poll(Duration::from_millis(10), Duration::from_millis(30)); // let grow run

    let labels = rendered_labels(&test);
    assert!(labels.contains(&"Item 1000".to_string()));
    assert!(labels.contains(&"Item 999".to_string()));
}

#[test]
fn last_item_growth_keeps_bottom() {
    let heights = Rc::new(RefCell::new(vec![50.0f32; 1000]));
    let mut bump = State::create_global(0u32);
    let app_heights = heights.clone();
    let app_bump = bump;
    let app = move || -> Element {
        let _ = *app_bump.read();
        let items = (0..1000).map(|i| format!("Item {i}")).collect::<Vec<_>>();
        let heights = app_heights.clone();
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                VirtualList::new(items, move |i, text, _measured_h| {
                    let h = heights.borrow()[i];
                    label().key(i).height(Size::px(h)).text(text.clone()).into()
                })
                .default_item_height(50.)
                .stick_to_bottom(true),
            )
            .into()
    };
    let mut test = launch_test(app);
    test.sync_and_update();
    assert!(rendered_labels(&test).contains(&"Item 999".to_string()));

    // The last item grows from 50px to 300px; the viewport must stay glued.
    let mut h = heights.borrow_mut();
    h[999] = 300.0;
    drop(h);
    *bump.write() += 1;
    test.sync_and_update();

    let labels = rendered_labels(&test);
    assert!(
        labels.contains(&"Item 999".to_string()),
        "viewport should stay at the bottom after the last item grows: {labels:?}"
    );
}

#[test]
fn scrolling_up_stops_following() {
    let (items, mut bump, app) = stick_list_app(1000, 50.);
    let mut test = launch_test(app);
    test.sync_and_update();

    // Scroll up (positive delta = towards older content), leaving the bottom.
    test.scroll((5., 5.), (0., 200.));
    test.sync_and_update();
    let before = rendered_labels(&test);
    let first: usize = before[0].strip_prefix("Item ").unwrap().parse().unwrap();
    // Just above the very bottom (moved ~200px up), so no long-jump happened.
    assert!(
        (980..998).contains(&first),
        "expected to be just above the bottom, got first item: {first}"
    );

    // Appending must NOT move the viewport while the user is scrolled up.
    items.borrow_mut().push("Item 1000".to_string());
    *bump.write() += 1;
    test.sync_and_update();
    test.poll(Duration::from_millis(10), Duration::from_millis(30)); // let grow run
    assert_eq!(rendered_labels(&test), before);
}

#[test]
fn scrolling_back_to_bottom_resumes() {
    let (items, mut bump, app) = stick_list_app(1000, 50.);
    let mut test = launch_test(app);
    test.sync_and_update();

    // Leave the bottom, then scroll all the way back down.
    test.scroll((5., 5.), (0., 200.));
    test.sync_and_update();
    test.scroll((5., 5.), (0., -100000.));
    test.sync_and_update();

    // Appending now follows again.
    items.borrow_mut().push("Item 1000".to_string());
    *bump.write() += 1;
    test.sync_and_update();
    test.poll(Duration::from_millis(10), Duration::from_millis(30)); // let grow run
    assert!(rendered_labels(&test).contains(&"Item 1000".to_string()));
}
