pub struct Theme {
    pub bg: [&'static str; 3],
    pub title: &'static str,
    pub subtitle: &'static str,
    pub footer: &'static str,
    pub dot_colors: &'static [&'static str],
    pub series_colors: &'static [&'static str],
    pub number: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub track: &'static str,
    pub value: &'static str,
    pub rme: &'static str,
    pub card: &'static str,
    pub card_opacity: f32,
    pub shadow: &'static str,
    pub shadow_opacity: f32,
    pub star: &'static str,
    pub sparkle: &'static str,
    pub kaomoji: &'static str,
}

pub const PASTEL: Theme = Theme {
    bg: ["#FFF8F0", "#FFF0F6", "#EEF7FF"],
    title: "#4A3542",
    subtitle: "#8A7080",
    footer: "#A88A98",
    dot_colors: &[
        "#FFB7C5", "#A8E6CF", "#BDE0FE", "#FFD6A5", "#CDB4DB", "#FFC8DD", "#B9FBC0",
    ],
    series_colors: &["#F4A6BE", "#7FD1B9", "#8CB8F0", "#F2B984", "#B49BE0"],
    number: "#6B4A5B",
    label: "#4A3542",
    note: "#9A7F8D",
    track: "#F3E5EC",
    value: "#4A3542",
    rme: "#A88A98",
    card: "#FFFFFF",
    card_opacity: 0.78,
    shadow: "#C084A0",
    shadow_opacity: 0.18,
    star: "#FFD166",
    sparkle: "#F4A7B9",
    kaomoji: "#B07C96",
};

pub const DARK: Theme = Theme {
    bg: ["#241C26", "#1F1A2B", "#191F2B"],
    title: "#FAEDF3",
    subtitle: "#C9B4C2",
    footer: "#9E8A99",
    dot_colors: &[
        "#FFB7C5", "#A8E6CF", "#BDE0FE", "#FFD6A5", "#CDB4DB", "#FFC8DD", "#B9FBC0",
    ],
    series_colors: &["#FF9EBC", "#7FE0C4", "#93C5FD", "#FFC98B", "#C4A9F0"],
    number: "#3B2B36",
    label: "#FAEDF3",
    note: "#BBA3B2",
    track: "#3A3140",
    value: "#FAEDF3",
    rme: "#B49EAC",
    card: "#3A3243",
    card_opacity: 1.0,
    shadow: "#000000",
    shadow_opacity: 0.45,
    star: "#FFD166",
    sparkle: "#F4A7B9",
    kaomoji: "#E3B6C9",
};

pub fn theme(name: &str) -> Option<&'static Theme> {
    match name {
        "pastel" | "" => Some(&PASTEL),
        "dark" | "pastel-dark" => Some(&DARK),
        _ => None,
    }
}
