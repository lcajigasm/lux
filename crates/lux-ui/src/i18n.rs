//! English and Spanish string catalog plus locale date/number formats.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Es,
}

impl Locale {
    pub fn parse(tag: &str) -> Self {
        let t = tag.to_ascii_lowercase();
        if t.starts_with("es") {
            Locale::Es
        } else {
            Locale::En
        }
    }

    pub fn t(self, key: &str) -> &'static str {
        match (self, key) {
            (Locale::En, "command.palette") => "Command Palette",
            (Locale::Es, "command.palette") => "Paleta de comandos",
            (Locale::En, "search.find") => "Find",
            (Locale::Es, "search.find") => "Buscar",
            (Locale::En, "git.changes") => "Changes",
            (Locale::Es, "git.changes") => "Cambios",
            (Locale::En, "term.panel") => "Terminal",
            (Locale::Es, "term.panel") => "Terminal",
            (Locale::En, "problems.panel") => "Problems",
            (Locale::Es, "problems.panel") => "Problemas",
            _ => "",
        }
    }

    pub fn format_int(self, n: i64) -> String {
        match self {
            Locale::En => n.to_string(),
            Locale::Es => {
                let s = n.abs().to_string();
                let mut out = String::new();
                for (i, c) in s.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        out.push('.');
                    }
                    out.push(c);
                }
                let mut grouped: String = out.chars().rev().collect();
                if n < 0 {
                    grouped.insert(0, '-');
                }
                grouped
            }
        }
    }

    pub fn format_iso_date(self, y: i32, m: u8, d: u8) -> String {
        match self {
            Locale::En => format!("{y:04}-{m:02}-{d:02}"),
            Locale::Es => format!("{d:02}/{m:02}/{y:04}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanish_and_english_catalog() {
        assert_eq!(Locale::En.t("command.palette"), "Command Palette");
        assert_eq!(Locale::Es.t("command.palette"), "Paleta de comandos");
        assert_eq!(Locale::Es.format_int(1234), "1.234");
        assert_eq!(Locale::Es.format_iso_date(2026, 9, 1), "01/09/2026");
        assert_eq!(Locale::En.format_iso_date(2026, 9, 1), "2026-09-01");
    }
}
