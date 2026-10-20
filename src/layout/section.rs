//! Section grouping: rows sharing a `group` are drawn together.

use super::RowPlan;

/// Rows grouped by their `group` field, in first-seen order. Rows without a
/// group form one untitled block.
pub(crate) fn sections_of(rows: &[RowPlan]) -> Vec<(Option<String>, Vec<usize>)> {
    let mut sections: Vec<(Option<String>, Vec<usize>)> = Vec::new();
    let mut index: hashbrown::HashMap<Option<String>, usize> = hashbrown::HashMap::default();
    for (row_index, row) in rows.iter().enumerate() {
        let key = row.group.clone();
        let slot = *index.entry(key.clone()).or_insert_with(|| {
            sections.push((key, Vec::new()));
            sections.len() - 1
        });
        sections[slot].1.push(row_index);
    }
    sections
}
