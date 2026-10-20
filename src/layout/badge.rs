//! Number badge geometry: a cat whose body stretches with the digit count.

use crate::font::FontStack;

use super::{NUMBER_BADGE_HEIGHT, NUMBER_SIZE, Rect, number_badge_width};

pub(crate) struct BadgeGeometry {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) ink_width: f32,
}

/// Each row's badge is as wide as its own digits, then centred in the slot.
pub(crate) fn badge_geometry(
    card: Rect,
    slot_width: f32,
    ordinal: usize,
    font: &FontStack,
) -> BadgeGeometry {
    let digits = ordinal.to_string();
    let ink = font.text_ink(&digits, NUMBER_SIZE);
    let ink_width = ink.right - ink.left;
    let w = number_badge_width(ink_width);
    BadgeGeometry {
        x: card.x + 14.0 + (slot_width - w) / 2.0,
        y: card.y + (card.h - NUMBER_BADGE_HEIGHT) / 2.0,
        w,
        h: NUMBER_BADGE_HEIGHT,
        ink_width,
    }
}
