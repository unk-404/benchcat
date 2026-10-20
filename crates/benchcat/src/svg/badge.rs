//! The cat that backs a row number.

use super::writer::Writer;

/// The row number sits on a solid cat in profile: body, head with merged ears,
/// tail and one paw. Everything is drawn in a 30px-high design space and scaled
/// to the badge, so the same shape works for a 33px badge and a 59px one.
///
/// Kept as a string entry point for the specimen example; the card renderer
/// calls [`draw_cat`] and keeps writing into its own buffer.
pub fn number_cat(svg: &mut String, pill: crate::layout::Rect, color: &str) {
    let mut out = Writer::with_capacity(512);
    draw_cat(&mut out, pill, color);
    svg.push_str(&out.finish());
}

pub(crate) fn draw_cat(out: &mut Writer, pill: crate::layout::Rect, color: &str) {
    let scale = pill.h / 30.0;
    let w = pill.w / scale;
    let bx = (w - 21.0).max(14.0);
    out.open("g")
        .attr_fmt(
            "transform",
            format_args!("translate({:.2} {:.2}) scale({scale:.4})", pill.x, pill.y),
        )
        .close();
    // Tail.
    out.open("path")
        .attr_raw("d", "M 6 24 C 2 24 0 19 3 16")
        .attr_raw("fill", "none")
        .attr_raw("stroke", color)
        .attr_raw("stroke-width", "2.8")
        .attr_raw("stroke-linecap", "round")
        .empty();
    // Body: flat back, a small dip, then a neck that climbs into the head.
    out.open("path")
        .attr_fmt(
            "d",
            format_args!(
                "M 6 27 C 2 27 1 22 1 18 C 1 12 7 10 14 10 L {bx:.2} 10 \
                 C {c1:.2} 10 {c2:.2} 8.6 {e:.2} 8 L {r:.2} 20 L {r:.2} 27 Z",
                c1 = bx + (w - 10.0 - bx) * 0.45,
                c2 = w - 13.0,
                e = w - 10.0,
                r = w - 4.0,
            ),
        )
        .attr_raw("fill", color)
        .empty();
    // Head with merged ears, drawn at 0.9 of the reference outline.
    out.open("g")
        .attr_fmt(
            "transform",
            format_args!("translate({:.2} 10) scale(0.9) translate(-35 -10)", w - 9.0),
        )
        .close();
    out.open("path")
        .attr_raw(
            "d",
            "M 29 5.5 L 28 -0.5 L 34 3.5 L 40.5 0 L 41 5.5 C 43 8.5 43 15 38.5 16.5 \
             C 34 18 30 17 28.5 12 C 27.5 8.5 28 6.5 29 5.5 Z",
        )
        .attr_raw("fill", color)
        .empty();
    out.end("g");
    out.end("g");
}
