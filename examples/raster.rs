//! Rasterise an SVG to PNG so the rendered card can be inspected in a diff.
//!
//! Usage: cargo run --example raster -- assets/benchmarks.svg target/preview.png [scale]

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let svg_path = PathBuf::from(args.get(1).cloned().unwrap_or_default());
    let png_path = PathBuf::from(args.get(2).cloned().unwrap_or_default());
    let scale: f32 = args
        .get(3)
        .and_then(|value| value.parse().ok())
        .unwrap_or(2.0);
    if svg_path.as_os_str().is_empty() || png_path.as_os_str().is_empty() {
        return Err("usage: raster <in.svg> <out.png> [scale]".into());
    }

    let svg = std::fs::read_to_string(&svg_path)?;
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(&svg, &options)?;
    let size = tree.size();
    let width = (size.width() * scale).ceil() as u32;
    let height = (size.height() * scale).ceil() as u32;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or("cannot allocate pixmap")?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    if let Some(parent) = png_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    pixmap.save_png(&png_path)?;
    println!(
        "{} -> {} ({}x{})",
        svg_path.display(),
        png_path.display(),
        width,
        height
    );
    Ok(())
}
