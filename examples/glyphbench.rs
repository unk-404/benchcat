//! Time outline extraction, with and without rayon, over every glyph the face maps.
//!
//! Usage: cargo run --release --example glyphbench -- fonts/mplus-rounded.ttf

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use bench_svg::Font;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "fonts/mplus-rounded.ttf".to_owned());
    let font = Font::load(Path::new(&path))?;

    let mut seen = HashSet::new();
    let mut gids = Vec::new();
    for codepoint in 0x20u32..=0x2FFFF {
        let Some(ch) = char::from_u32(codepoint) else {
            continue;
        };
        if let Some(glyph) = font.glyph(ch)
            && seen.insert(glyph.gid)
        {
            gids.push(glyph.gid);
        }
    }

    let mut best = f64::MAX;
    let mut total_bytes = 0usize;
    for _ in 0..5 {
        let start = Instant::now();
        let paths = font.extract_paths(&gids);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        best = best.min(elapsed);
        total_bytes = paths.iter().map(String::len).sum();
    }

    println!(
        "{} glyphs · {:.1} KiB of path data · best {:.2} ms ({:.1} k glyphs/s)",
        gids.len(),
        total_bytes as f64 / 1024.0,
        best,
        gids.len() as f64 / best
    );
    Ok(())
}
