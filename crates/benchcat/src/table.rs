/// Table input shared by v2 table variants.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn cell(&self, row: &[String], name: &str) -> Option<String> {
        let index = self.headers.iter().position(|header| header == name)?;
        row.get(index)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    pub fn measurement_value(&self, row: &[String], name: &str) -> Option<f64> {
        self.cell(row, name)
            .and_then(|value| measurement_value(&value))
    }

    pub fn measurement_unit(&self, row: &[String], name: &str) -> Option<String> {
        self.cell(row, name)
            .and_then(|value| measurement_unit(&value))
    }
}

pub fn header_contains(text: &str, header: &str) -> bool {
    text.lines().take(8).any(|line| line.contains(header))
}

pub fn parse_markdown_table(text: &str) -> Option<Table> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let header_index = lines
        .iter()
        .position(|line| line.starts_with('|') && line.to_ascii_lowercase().contains("mean"))?;
    let headers = split_markdown_row(lines[header_index])?;
    let separator = lines.get(header_index + 1)?;
    if !separator.starts_with('|') || !separator.contains("---") {
        return None;
    }
    let mut rows = Vec::new();
    for line in &lines[header_index + 2..] {
        if !line.starts_with('|') {
            continue;
        }
        if let Some(row) = split_markdown_row(line) {
            if row.len() >= headers.len() {
                rows.push(row);
            }
        }
    }
    Some(Table { headers, rows })
}

fn split_markdown_row(line: &str) -> Option<Vec<String>> {
    let line = line.trim().trim_start_matches('|').trim_end_matches('|');
    let values: Vec<String> = line
        .split('|')
        .map(|value| value.trim().to_owned())
        .collect();
    (!values.is_empty()).then_some(values)
}

pub fn parse_csv_table(text: &str) -> Option<Table> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader.headers().ok()?.iter().map(str::to_owned).collect();
    let mut rows = Vec::new();
    for row in reader.records() {
        let row = row.ok()?;
        rows.push(row.iter().map(str::to_owned).collect());
    }
    Some(Table { headers, rows })
}

pub fn measurement_value(text: &str) -> Option<f64> {
    let cleaned = text.replace(',', "");
    let mut current = String::new();
    for ch in cleaned.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' {
            current.push(ch);
        } else if !current.is_empty() {
            break;
        }
    }
    current.parse().ok()
}

pub fn measurement_unit(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let token =
            token.trim_matches(|ch: char| !ch.is_ascii_alphabetic() && ch != 'µ' && ch != 'μ');
        if matches!(token, "ns" | "us" | "µs" | "μs" | "ms" | "s") {
            return Some(token.to_owned());
        }
    }
    None
}
