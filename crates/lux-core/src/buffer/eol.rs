/// A line-ending style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    Lf,
    Crlf,
    Cr,
}

impl Eol {
    pub fn as_str(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
            Eol::Cr => "\r",
        }
    }

    /// Detects the predominant line ending in `text` by counting each style
    /// and picking the majority. Defaults to `Lf` (the project's own style)
    /// when the text has no line ending at all.
    pub fn detect(text: &str) -> Eol {
        let bytes = text.as_bytes();
        let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                    crlf += 1;
                    i += 2;
                }
                b'\r' => {
                    cr += 1;
                    i += 1;
                }
                b'\n' => {
                    lf += 1;
                    i += 1;
                }
                _ => i += 1,
            }
        }
        if crlf >= lf && crlf >= cr && crlf > 0 {
            Eol::Crlf
        } else if cr > lf {
            Eol::Cr
        } else {
            Eol::Lf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_lf() {
        assert_eq!(Eol::detect("a\nb\nc\n"), Eol::Lf);
    }

    #[test]
    fn detects_crlf() {
        assert_eq!(Eol::detect("a\r\nb\r\nc\r\n"), Eol::Crlf);
    }

    #[test]
    fn detects_cr() {
        assert_eq!(Eol::detect("a\rb\rc\r"), Eol::Cr);
    }

    #[test]
    fn defaults_to_lf_with_no_eol() {
        assert_eq!(Eol::detect("just one line"), Eol::Lf);
    }

    #[test]
    fn picks_majority_style() {
        assert_eq!(Eol::detect("a\nb\nc\r\n"), Eol::Lf);
        assert_eq!(Eol::detect("a\r\nb\r\nc\n"), Eol::Crlf);
    }

    #[test]
    fn as_str_round_trips() {
        assert_eq!(Eol::Lf.as_str(), "\n");
        assert_eq!(Eol::Crlf.as_str(), "\r\n");
        assert_eq!(Eol::Cr.as_str(), "\r");
    }
}
