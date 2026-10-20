//! Orchestration: canonical models in, one SVG string out.

use std::path::Path;
use std::time::Instant;

use crate::config::Config;
use crate::font::{Font, FontStack};
use crate::layout::{LayoutOptions, LegendItem, layout};
use crate::model::{Benchmark, Meta};
use crate::svg::{self, RenderInput};
use crate::template::{self, Context as TemplateContext};
use tracing::debug;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error(transparent)]
    Layout(#[from] crate::layout::LayoutError),
    #[error("theme {0:?} is unknown; expected \"pastel\" or \"dark\"")]
    UnknownTheme(String),
    #[error("{0}")]
    Rule(String),
}

pub struct Rendered {
    pub svg: String,
    pub warnings: Vec<String>,
    pub rows: usize,
}

/// One source of measurements: a platform, a language, a branch, ...
pub struct SeriesInput<'a> {
    pub label: Option<String>,
    pub color: Option<String>,
    pub benchmarks: &'a [Benchmark],
}

pub struct RenderRequest<'a> {
    pub config: &'a Config,
    pub meta: &'a Meta,
    pub series: &'a [SeriesInput<'a>],
    pub font: &'a FontStack,
}

pub fn render(request: RenderRequest<'_>) -> Result<Rendered, RenderError> {
    let RenderedParts {
        prefix,
        rows,
        suffix,
        warnings,
        row_count,
    } = render_parts(request)?;
    let mut svg = prefix;
    for chunk in rows {
        svg.push_str(&chunk);
    }
    svg.push_str(&suffix);
    Ok(Rendered {
        svg,
        warnings,
        rows: row_count,
    })
}

/// The card split at the row boundary, for callers that write the pieces out.
pub struct RenderedParts {
    pub prefix: String,
    pub rows: Vec<String>,
    pub suffix: String,
    pub warnings: Vec<String>,
    pub row_count: usize,
}

impl RenderedParts {
    pub fn len(&self) -> usize {
        self.prefix.len() + self.rows.iter().map(String::len).sum::<usize>() + self.suffix.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub fn render_parts(request: RenderRequest<'_>) -> Result<RenderedParts, RenderError> {
    let render = &request.config.render;
    let theme =
        svg::theme(&render.theme).ok_or_else(|| RenderError::UnknownTheme(render.theme.clone()))?;
    let multi_series = request.series.len() > 1;

    let started = Instant::now();

    let legend: Vec<LegendItem> = if multi_series {
        request
            .series
            .iter()
            .enumerate()
            .map(|(index, series)| LegendItem {
                label: series
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("source {}", index + 1)),
                color: series.color.clone().unwrap_or_else(|| {
                    theme.series_colors[index % theme.series_colors.len()].to_owned()
                }),
            })
            .collect()
    } else {
        Vec::new()
    };

    let merge_start = Instant::now();
    let rows = merge_rows(request.series, multi_series, request.config)?;
    debug!(
        series = request.series.len(),
        rows = rows.len(),
        elapsed_ms = merge_start.elapsed().as_secs_f64() * 1_000.0,
        "merged sources"
    );
    let rows = match render.max_rows {
        Some(limit) if rows.len() > limit => &rows[..limit],
        _ => &rows[..],
    };

    // `{placeholder}` expansion keeps provenance out of the committed config.
    let context = TemplateContext {
        meta: request.meta,
        rows: rows.len(),
        font: request.font.family(),
    };
    let mut warnings: Vec<String> = Vec::new();
    let title_source = render
        .title
        .clone()
        .unwrap_or_else(|| "benchmarks".to_owned());
    let (title, title_warnings) = template::expand(&title_source, &context);
    warnings.extend(title_warnings);
    let subtitle_source = render
        .subtitle
        .clone()
        .unwrap_or_else(|| default_subtitle(request.meta));
    let (subtitle, subtitle_warnings) = template::expand(&subtitle_source, &context);
    warnings.extend(subtitle_warnings);
    let footer_source = render
        .footer
        .clone()
        .unwrap_or_else(|| default_footer(request.meta, rows.len()));
    let (footer, footer_warnings) = template::expand(&footer_source, &context);
    warnings.extend(footer_warnings);

    let options = LayoutOptions {
        width: render.width,
        min_height: render.min_height,
        card_height: render.card_height,
        row_gap: render.row_gap,
        padding: render.padding,
        title: title.clone(),
        subtitle: subtitle.clone(),
        footer: footer.clone(),
        legend,
    };
    let layout_start = Instant::now();
    let plan = layout(request.font, rows, &options)?;
    debug!(
        elapsed_ms = layout_start.elapsed().as_secs_f64() * 1_000.0,
        "laid out card"
    );
    let svg_start = Instant::now();
    let output = svg::render_parts(
        request.font,
        &plan,
        &RenderInput {
            title: &title,
            subtitle: &subtitle,
            footer: &footer,
            kaomoji: &render.kaomoji,
        },
        theme,
    );

    let bytes = output.prefix.len()
        + output.rows.iter().map(String::len).sum::<usize>()
        + output.suffix.len();
    debug!(
        bytes,
        elapsed_ms = svg_start.elapsed().as_secs_f64() * 1_000.0,
        "wrote svg tree"
    );
    debug!(
        total_ms = started.elapsed().as_secs_f64() * 1_000.0,
        "render complete"
    );

    Ok(RenderedParts {
        prefix: output.prefix,
        rows: output.rows,
        suffix: output.suffix,
        warnings: {
            warnings.extend(output.warnings);
            warnings
        },
        row_count: rows.len(),
    })
}

mod merge;
mod meta;

use merge::merge_rows;
use meta::{default_footer, default_subtitle};

/// Load the font, preferring an explicit file, then a family name, then `fonts/`.
pub fn load_font(
    explicit: Option<&Path>,
    family: Option<&str>,
) -> Result<Font, crate::font::FontError> {
    let path = crate::font::resolve_font(explicit, family)?;
    Font::load(&path)
}
