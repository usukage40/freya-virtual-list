//! A performant virtual list component for [Freya].
//!
//! Renders only the items currently visible inside the viewport (plus an
//! overscan margin), supports **variable item heights** by measuring each
//! rendered item at runtime, and keeps the scroll position stable when item
//! heights above the viewport are corrected.
//!
//! [Freya]: https://freyaui.dev/

pub mod virtual_list;

pub use virtual_list::VirtualList;
