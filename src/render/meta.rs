//! Default subtitle and footer text.

use crate::model::Meta;

pub(crate) fn default_subtitle(meta: &Meta) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(platform) = meta.platform.summary() {
        parts.push(platform);
    }
    if let Some(tool) = &meta.tool {
        let mut label = tool.clone();
        if let Some(version) = &meta.tool_version {
            label.push(' ');
            label.push_str(version);
        }
        parts.push(label);
    }
    if let Some(commit) = &meta.commit {
        parts.push(format!("commit {}", short_commit(commit)));
    }
    if parts.is_empty() {
        "benchcat".to_owned()
    } else {
        parts.join(" · ")
    }
}

pub(crate) fn default_footer(meta: &Meta, rows: usize) -> String {
    let mut text = format!(
        "Generated from {rows} benchmark {}",
        if rows == 1 { "row" } else { "rows" }
    );
    if let Some(tool) = &meta.tool {
        text.push_str(" · ");
        text.push_str(tool);
    }
    if let Some(commit) = &meta.commit {
        text.push_str(" · ");
        text.push_str(short_commit(commit));
    }
    text.push_str(" · bars are relative to the best value in each unit");
    text
}

fn short_commit(commit: &str) -> &str {
    if commit.len() > 7 {
        &commit[..7]
    } else {
        commit
    }
}
