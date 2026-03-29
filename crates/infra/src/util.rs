use regex::Regex;
use std::sync::LazyLock;

static NON_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\w\s-]").unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s_]+").unwrap());
static MULTI_DASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-+").unwrap());

pub fn slugify(text: &str) -> String {
    let text = text.to_lowercase();
    let text = text.trim();
    let text = NON_WORD_RE.replace_all(text, "");
    let text = WHITESPACE_RE.replace_all(&text, "-");
    let text = MULTI_DASH_RE.replace_all(&text, "-");
    let text = text.trim_matches('-');
    if text.len() > 80 {
        text[..80].trim_end_matches('-').to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn special_chars_removed() {
        assert_eq!(slugify("Hello! @World#"), "hello-world");
    }

    #[test]
    fn truncates_at_80() {
        let long = "a".repeat(100);
        assert!(slugify(&long).len() <= 80);
    }

    #[test]
    fn handles_empty() {
        assert_eq!(slugify(""), "");
    }
}
