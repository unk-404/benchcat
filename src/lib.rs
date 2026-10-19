//! bench-svg: render benchmark results as a self-contained pastel SVG card.
//!
//! The Dart renderer this replaces hard-coded its rows, its unit conversions and
//! its font atlas. This crate keeps the same visual language but takes the data,
//! the units and the typeface as inputs, and derives every measurement from the
//! font file it actually draws with.

pub mod adapters;
pub mod config;
pub mod expr;
pub mod font;
pub mod input;
pub mod layout;
pub mod model;
pub mod render;
pub mod rows;
pub mod svg;
pub mod template;

pub use config::Config;
pub use font::Font;
pub use input::{Format, Loaded};
pub use model::{Benchmark, Report, Unit};
