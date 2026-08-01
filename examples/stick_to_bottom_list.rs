#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

//! Example: a bottom-sticking `VirtualList` that follows appended items.
//!
//! Run with:
//! ```sh
//! cargo run --example stick_to_bottom_list
//! ```
//!
//! The list starts at the bottom (like a chat history / live log). Press
//! **Space** to append a random text item: while you are at the bottom the
//! viewport stays glued to the newest content; scroll up to pause following
//! and the viewport stays put; scroll back to the bottom to resume.

use freya::prelude::*;
use freya_virtual_list::VirtualList;

/// A tiny deterministic PRNG so the demo needs no extra dependencies.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
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

const WORDS: &[&str] = &[
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

fn seed_for(i: usize) -> u64 {
    0x9E37_79B9 ^ (i as u64).wrapping_mul(0x517C_C1B7)
}

fn app() -> impl IntoElement {
    let mut items = use_state(|| {
        (0..5000)
            .map(|i| random_text(seed_for(i), WORDS))
            .collect::<Vec<_>>()
    });

    let items_vec = items.read().clone();

    // Append a random item on Space (global, no focus needed).
    let on_key = move |e: Event<KeyboardEventData>| {
        if e.data().key == Key::Character(" ".to_string()) {
            let i = items.read().len();
            items.write().push(random_text(seed_for(i), WORDS));
        }
    };

    rect()
        .width(Size::fill())
        .height(Size::fill())
        .background((24, 24, 28))
        .padding(8.)
        .on_global_key_down(on_key)
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
            .overscan(4)
            .stick_to_bottom(true),
        )
}

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app)))
}
