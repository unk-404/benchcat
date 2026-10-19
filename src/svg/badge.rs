//! The cat that backs a row number.

/// The row number sits on a solid cat in profile: body, head, ears, tail and two
/// legs. The body stretches with the digit count; everything else is fixed.
/// The row number sits on a cat: body, head with merged ears, tail and one paw.
/// Everything is drawn in a 30px-high design space and scaled to the badge, so
/// the same shape works for a 33px badge and a 59px one.
pub fn number_cat(svg: &mut String, pill: crate::layout::Rect, color: &str) {
    let scale = pill.h / 30.0;
    let w = pill.w / scale;
    let bx = (w - 21.0).max(14.0);
    svg.push_str(&format!(
        "<g transform=\"translate({x:.2} {y:.2}) scale({scale:.4})\">",
        x = pill.x,
        y = pill.y
    ));
    // Tail.
    svg.push_str(&format!(
        "<path d=\"M 6 24 C 2 24 0 19 3 16\" fill=\"none\" stroke=\"{color}\" \
         stroke-width=\"2.8\" stroke-linecap=\"round\"/>"
    ));
    // Body: flat back, a small dip, then a neck that climbs into the head.
    svg.push_str(&format!(
        "<path d=\"M 6 27 C 2 27 1 22 1 18 C 1 12 7 10 14 10 L {bx:.2} 10 \
         C {c1:.2} 10 {c2:.2} 8.6 {e:.2} 8 L {r:.2} 20 L {r:.2} 27 Z\" fill=\"{color}\"/>",
        c1 = bx + (w - 10.0 - bx) * 0.45,
        c2 = w - 13.0,
        e = w - 10.0,
        r = w - 4.0,
    ));
    // Head with merged ears, drawn at 0.9 of the reference outline.
    svg.push_str(&format!(
        "<g transform=\"translate({hx:.2} 10) scale(0.9) translate(-35 -10)\">\
         <path d=\"M 29 5.5 L 28 -0.5 L 34 3.5 L 40.5 0 L 41 5.5 C 43 8.5 43 15 38.5 16.5 \
         C 34 18 30 17 28.5 12 C 27.5 8.5 28 6.5 29 5.5 Z\" fill=\"{color}\"/></g>",
        hx = w - 9.0
    ));
    svg.push_str("</g>");
}
