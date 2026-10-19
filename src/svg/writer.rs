//! One place that turns elements, attributes and text into SVG bytes.
//!
//! Nothing here builds intermediate `format!` strings: every write goes
//! straight into the output buffer, attributes are escaped in one pass, and
//! numbers reuse the same fixed-precision formatting as the rest of the crate
//! so existing assets stay byte-identical.

use std::fmt::{self, Write as _};

pub(crate) struct Writer {
    buf: String,
}

impl Writer {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: String::with_capacity(capacity),
        }
    }

    /// `<name`; attributes follow, then [`Writer::close`] or [`Writer::empty`].
    pub(crate) fn open(&mut self, name: &str) -> &mut Self {
        self.buf.push('<');
        self.buf.push_str(name);
        self
    }

    /// ` name="value"`, with XML escaping.
    pub(crate) fn attr(&mut self, name: &str, value: &str) -> &mut Self {
        self.buf.push(' ');
        self.buf.push_str(name);
        self.buf.push_str("=\"");
        escape_into(&mut self.buf, value);
        self.buf.push('"');
        self
    }

    /// ` name="value"` for values this crate generated itself (colours, paths,
    /// ids); no escaping is attempted.
    pub(crate) fn attr_raw(&mut self, name: &str, value: &str) -> &mut Self {
        self.buf.push(' ');
        self.buf.push_str(name);
        self.buf.push_str("=\"");
        self.buf.push_str(value);
        self.buf.push('"');
        self
    }

    /// ` name="v"` with exactly `decimals` digits after the point.
    ///
    /// Whole numbers (most geometry) skip the formatting machinery entirely
    /// while printing the same `"12.00"` a fixed-precision format would.
    pub(crate) fn attr_num(&mut self, name: &str, value: f32, decimals: usize) -> &mut Self {
        self.buf.push(' ');
        self.buf.push_str(name);
        self.buf.push_str("=\"");
        let whole = value.fract() == 0.0 && value.abs() < 1e9;
        if whole {
            let rounded = value as i64;
            if rounded == 0 {
                self.buf.push('0');
            } else {
                push_i64(&mut self.buf, rounded);
            }
            if decimals > 0 {
                self.buf.push('.');
                for _ in 0..decimals {
                    self.buf.push('0');
                }
            }
        } else {
            let _ = write!(self.buf, "{value:.decimals$}");
        }
        self.buf.push('"');
        self
    }

    /// ` name="prefix{id}"`, for `#g12`, `g12` and similar short ids.
    pub(crate) fn attr_id(&mut self, name: &str, prefix: &str, id: usize) -> &mut Self {
        self.buf.push(' ');
        self.buf.push_str(name);
        self.buf.push_str("=\"");
        self.buf.push_str(prefix);
        push_usize(&mut self.buf, id);
        self.buf.push('"');
        self
    }

    /// ` name="v"` where `v` is a formula too small to be worth a temporary.
    pub(crate) fn attr_fmt(&mut self, name: &str, args: fmt::Arguments<'_>) -> &mut Self {
        self.buf.push(' ');
        self.buf.push_str(name);
        self.buf.push_str("=\"");
        self.buf.write_fmt(args).expect("string writes cannot fail");
        self.buf.push('"');
        self
    }

    /// Ends the start tag with `>`.
    pub(crate) fn close(&mut self) -> &mut Self {
        self.buf.push('>');
        self
    }

    /// Ends the start tag with `/>`.
    pub(crate) fn empty(&mut self) -> &mut Self {
        self.buf.push_str("/>");
        self
    }

    /// Escaped text content.
    pub(crate) fn text(&mut self, text: &str) -> &mut Self {
        escape_into(&mut self.buf, text);
        self
    }

    /// Verbatim markup the writer cannot express (the XML declaration).
    pub(crate) fn raw(&mut self, markup: &str) -> &mut Self {
        self.buf.push_str(markup);
        self
    }

    pub(crate) fn end(&mut self, name: &str) -> &mut Self {
        self.buf.push_str("</");
        self.buf.push_str(name);
        self.buf.push('>');
        self
    }

    pub(crate) fn finish(self) -> String {
        self.buf
    }
}

/// Escapes the five XML specials in one pass; the common case (no specials)
/// copies the whole string without looking at individual chars.
fn escape_into(out: &mut String, text: &str) {
    let mut last = 0;
    for (index, ch) in text.char_indices() {
        let replacement = match ch {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&apos;",
            _ => continue,
        };
        out.push_str(&text[last..index]);
        out.push_str(replacement);
        last = index + ch.len_utf8();
    }
    out.push_str(&text[last..]);
}

/// Signed decimal digits without going through `fmt`.
fn push_i64(out: &mut String, value: i64) {
    if value < 0 {
        out.push('-');
    }
    push_usize(out, value.unsigned_abs() as usize);
}

/// Decimal digits without going through `fmt`.
fn push_usize(out: &mut String, mut value: usize) {
    let mut digits = [0u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    out.push_str(std::str::from_utf8(&digits[start..]).expect("ASCII digits"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_and_attributes() {
        let mut w = Writer::with_capacity(64);
        w.open("title").close().text("a & b < c \"d\"").end("title");
        w.open("rect")
            .attr("aria-label", "a & b")
            .attr_num("x", 1.5, 1)
            .empty();
        assert_eq!(
            w.finish(),
            "<title>a &amp; b &lt; c &quot;d&quot;</title><rect aria-label=\"a &amp; b\" x=\"1.5\"/>"
        );
    }

    #[test]
    fn writes_ids_without_allocating() {
        let mut w = Writer::with_capacity(16);
        w.open("use")
            .attr_id("href", "#g", 123)
            .attr_num("x", 40.0, 1)
            .empty();
        assert_eq!(w.finish(), "<use href=\"#g123\" x=\"40.0\"/>");
    }
}
