//! SVG emission for one laid-out plan.

mod atlas;
mod badge;
mod emit;
mod theme;

pub use atlas::Anchor;
pub use badge::number_cat;
pub use emit::{RenderInput, RenderOutput, render};
pub use theme::{DARK, PASTEL, Theme, theme};
