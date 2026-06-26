use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, PartialEq)]
pub enum Directive {
    Skip,
    SkipStart,
    SkipEnd,
    Comment(String),
}

pub fn find(html: &str) -> Option<Directive> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        let pattern = r"(?x)
              <!-{2,}\s*                  # the opening of the comment
              (?:i18n|mdbook-xgettext)    # the prefix
              \s*:                        # delimit between prefix and command
              (?<command>.*[^-])          # the command part of the prefix
              -{2,}>                      # the closing of the comment
        ";
        Regex::new(pattern).expect("well-formed regex")
    });

    let captures = re.captures(html.trim())?;

    let command = captures["command"].trim();
    let mut tokens = command.split(is_delimiter).filter(|s| !s.is_empty());
    match (tokens.next(), tokens.next()) {
        (Some("skip"), None) => Some(Directive::Skip),
        (Some("skip"), Some("start")) => Some(Directive::SkipStart),
        (Some("skip"), Some("end")) => Some(Directive::SkipEnd),
        (Some("comment"), _) => {
            // "comment" is ASCII, so this offset is always on a char
            // boundary. The separator after the keyword may be a multi-byte
            // Unicode whitespace, so drop it via `chars()` rather than a fixed
            // byte offset to avoid slicing inside a code point.
            let after_keyword = command.find("comment").unwrap() + "comment".len();
            let rest = &command[after_keyword..];
            let body = match rest.chars().next() {
                Some(c) if is_delimiter(c) => &rest[c.len_utf8()..],
                _ => rest,
            };
            Some(Directive::Comment(body.trim().into()))
        }
        _ => None,
    }
}

fn is_delimiter(c: char) -> bool {
    c.is_whitespace() || c == ':' || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_comment_skip_directive_simple() {
        assert!(matches!(find("<!-- i18n:skip -->"), Some(Directive::Skip)));
    }

    #[test]
    fn test_is_comment_skip_directive_tolerates_spaces() {
        assert!(matches!(find("<!-- i18n: skip -->"), Some(Directive::Skip)));
    }

    #[test]
    fn test_is_comment_skip_directive_tolerates_dashes() {
        assert!(matches!(
            find("<!--- i18n:skip ---->"),
            Some(Directive::Skip)
        ));
    }

    #[test]
    fn test_is_comment_skip_directive_needs_skip() {
        assert!(find("<!-- i18n: foo -->").is_none());
    }

    #[test]
    fn test_is_comment_skip_directive_needs_to_be_a_comment() {
        assert!(find("<div>i18: skip</div>").is_none());
    }

    #[test]
    fn test_different_prefix() {
        assert!(matches!(
            find("<!-- mdbook-xgettext:skip -->"),
            Some(Directive::Skip)
        ));
    }

    #[test]
    fn test_skip_start_simple() {
        assert!(matches!(
            find("<!-- i18n:skip-start -->"),
            Some(Directive::SkipStart)
        ));
    }

    #[test]
    fn test_skip_start_tolerates_spaces() {
        assert!(matches!(
            find("<!-- i18n: skip-start -->"),
            Some(Directive::SkipStart)
        ));
    }

    #[test]
    fn test_skip_start_tolerates_dashes() {
        assert!(matches!(
            find("<!--- i18n:skip-start ---->"),
            Some(Directive::SkipStart)
        ));
    }

    #[test]
    fn test_skip_start_different_prefix() {
        assert!(matches!(
            find("<!-- mdbook-xgettext:skip-start -->"),
            Some(Directive::SkipStart)
        ));
    }

    #[test]
    fn test_skip_start_not_confused_with_skip() {
        assert!(!matches!(
            find("<!-- i18n:skip-start -->"),
            Some(Directive::Skip)
        ));
    }

    #[test]
    fn test_skip_end_simple() {
        assert!(matches!(
            find("<!-- i18n:skip-end -->"),
            Some(Directive::SkipEnd)
        ));
    }

    #[test]
    fn test_skip_end_tolerates_spaces() {
        assert!(matches!(
            find("<!-- i18n: skip-end -->"),
            Some(Directive::SkipEnd)
        ));
    }

    #[test]
    fn test_skip_end_different_prefix() {
        assert!(matches!(
            find("<!-- mdbook-xgettext:skip-end -->"),
            Some(Directive::SkipEnd)
        ));
    }

    #[test]
    fn test_comment() {
        assert!(match find("<!-- i18n:comment: hello world! -->") {
            Some(Directive::Comment(s)) => {
                s == "hello world!"
            }
            _ => false,
        });
    }

    #[test]
    fn test_empty_comment_does_nothing() {
        assert!(match find("<!-- i18n:comment -->") {
            Some(Directive::Comment(s)) => {
                s.is_empty()
            }
            _ => false,
        });
    }

    #[test]
    fn test_comment_multibyte_separator() {
        // A multi-byte Unicode whitespace (here a no-break space) separating
        // the keyword from the body must not be sliced through.
        assert!(match find("<!-- i18n:comment\u{a0}hello world! -->") {
            Some(Directive::Comment(s)) => {
                s == "hello world!"
            }
            _ => false,
        });
    }
}
