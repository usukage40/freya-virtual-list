# freya-virtual-list

A performant virtual list component for [Freya](https://freyaui.dev/).

Renders only the items currently visible inside the viewport (plus an overscan
margin), supports **variable item heights** by measuring each rendered item at
runtime, and keeps the scroll position stable when item heights above the
viewport are corrected.

## Features

- **Virtualized rendering** — only items in the viewport (plus overscan) are
  mounted, so lists of tens of thousands of items stay fast.
- **Variable item heights** — every rendered item is measured at runtime; no
  fixed-height assumption.
- **Stable scroll position** — scroll is anchored to the top item, so content
  doesn't jump when heights above the viewport are corrected.
- **O(1) height updates** — measured heights are tracked in chunks of 64 with
  prefix sums, instead of recomputing the whole list on every measurement.
- **Auto-hiding scrollbar** — draggable thumb, click-to-jump on the track, and
  a configurable idle delay after which the scrollbar fades out (200 ms
  animation) instead of disappearing instantly.

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
freya = { git = "https://github.com/marc2332/freya" }
freya-virtual-list = { git = "https://github.com/usukage40/freya-virtual-list" }
```

```rust
use freya::prelude::*;
use freya_virtual_list::VirtualList;

fn app() -> impl IntoElement {
    let items = use_state(|| vec![
        "Hello".to_string(),
        "World".to_string(),
        "Variable height items".to_string(),
    ]);

    let items_clone = items.read().clone();
    VirtualList::new(items_clone, move |index, item, _measured_h| {
        rect()
            .key(index)
            .height(Size::px(50.))
            .padding(4.)
            .child(format!("#{index}: {item}"))
            .into()
    })
    .default_item_height(50.)
    .item_gap(4.)
}
```

## API

`VirtualList<T, B>` renders a list of `Vec<T>` items with a render callback
`Fn(usize, &T, Option<f32>) -> Element` (item index, item, and its measured
height, if any).

| Builder method | Default | Description |
|---|---|---|
| `default_item_height(h: f32)` | `80.0` | Fallback height for items not yet measured. |
| `item_gap(gap: f32)` | `0.0` | Vertical gap between items. |
| `overscan(n: usize)` | `3` | Extra items rendered above/below the viewport. |
| `scrollbar_hide_delay(duration: Duration)` | `800ms` | How long the scrollbar stays visible after the last scroll/drag/hover, before it fades out. |

## Example

```sh
cargo run --example random_text_list
```

Renders 5,000 randomly generated text items with varying line counts,
demonstrating the variable-height support.

## Tests

```sh
cargo test
```

Runs unit tests for the height bookkeeping plus headless
[freya-testing](https://crates.io/crates/freya-testing) integration tests
covering virtualization, scrolling, scrollbar dragging and auto-hide.

## License

[Apache-2.0](LICENSE)
