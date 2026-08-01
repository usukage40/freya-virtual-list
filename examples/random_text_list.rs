#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

//! Example: a `VirtualList` showing randomly generated text items.
//!
//! Run with:
//! ```sh
//! cargo run --example random_text_list
//! ```
//!
//! Every item is a small text block whose height depends on the number of
//! lines it contains, demonstrating the list's variable-height support.

use freya::prelude::*;
use freya_virtual_list::VirtualList;

/// A tiny deterministic PRNG so the demo needs no extra dependencies.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        let span = (hi - lo + 1) as u64;
        lo + (self.next() % span) as usize
    }
}

fn random_text(seed: u64, words: &[&str]) -> String {
    let mut rng = Lcg(seed);
    let sentence_count = rng.range(1, 4);
    let mut text = String::new();
    for _ in 0..sentence_count {
        let word_count = rng.range(4, 14);
        for _ in 0..word_count {
            text.push_str(words[rng.range(0, words.len() - 1)]);
            text.push(' ');
        }
        text.pop();
        text.push_str(". ");
    }
    text
}

fn app() -> impl IntoElement {
    let items = use_state(|| {
        let words = [
            "rust",
            "freya",
            "skia",
            "virtual",
            "list",
            "variable",
            "height",
            "scroll",
            "viewport",
            "overscan",
            "render",
            "reactive",
            "state",
            "component",
            "element",
            "layout",
            "pixel",
            "measure",
            "prepend",
            "async",
        ];
        (0..5000)
            .map(|i| random_text(0x9E37_79B9 ^ (i as u64).wrapping_mul(0x517C_C1B7), &words))
            .collect::<Vec<_>>()
    });

    let items_vec = items.read().clone();

    rect()
        .width(Size::fill())
        .height(Size::fill())
        .background((24, 24, 28))
        .padding(8.)
        .child(
            VirtualList::new(items_vec, move |index, text, _measured_h| {
                let bg = (30 + (index % 5) as u8 * 8, 34, 40);
                rect()
                    .key(index)
                    .width(Size::fill())
                    .padding(10.)
                    .corner_radius(6.)
                    .background(bg)
                    .child(
                        rect()
                            .direction(Direction::Horizontal)
                            .cross_align(Alignment::Start)
                            .spacing(8.)
                            .child(
                                label()
                                    .font_size(12.)
                                    .color((255, 255, 255))
                                    .text(format!("#{index}")),
                            )
                            .child(
                                label()
                                    .font_size(14.)
                                    .color((255, 255, 255))
                                    .text(text.clone()),
                            ),
                    )
                    .into()
            })
            .default_item_height(48.)
            .item_gap(4.)
            .overscan(4),
        )
}

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app)))
}
