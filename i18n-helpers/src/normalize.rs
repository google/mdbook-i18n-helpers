//! Normalize the Markdown in a PO or POT file.
//!
//! This file contains main logic used by the binary `mdbook-i18n-normalize`.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use crate::{extract_messages, new_cmark_parser, wrap_sources};
use polib::catalog::Catalog;
use polib::message::{Message, MessageFlags, MessageMutView, MessageView};
use pulldown_cmark::{Event, LinkType, Tag};

fn parse_source(source: &str) -> Option<(&str, usize)> {
    let (path, lineno) = source.split_once(':')?;
    Some((path, lineno.parse().ok()?))
}

/// Use only the potion of extract_messages that the normalizer cares about.
fn extract_document_messages(doc: &str) -> Vec<(usize, String)> {
    extract_messages(doc)
        .into_iter()
        .map(|(idx, extracted)| (idx, extracted.message))
        .collect()
}

fn compute_source(source: &str, delta: usize) -> String {
    let mut new_source = String::with_capacity(source.len());

    for path_lineno in source.split_whitespace() {
        if !new_source.is_empty() {
            new_source.push('\n');
        }
        if let Some((path, lineno)) = parse_source(path_lineno) {
            new_source.push_str(&format!("{path}:{}", lineno + delta));
        } else {
            new_source.push_str(source);
        }
    }

    wrap_sources(&new_source)
}

/// Check if `text` contains one or more broken reference links.
fn has_broken_link(text: &str) -> bool {
    // The return value from the callback is not important, it just
    // has to return Some to generate a `LinkType::*Unknown`.
    let mut callback = |_| Some(("".into(), "".into()));
    new_cmark_parser(text, Some(&mut callback)).any(|event| {
        matches!(
            event,
            Event::Start(Tag::Link {
                link_type: LinkType::ReferenceUnknown
                    | LinkType::CollapsedUnknown
                    | LinkType::ShortcutUnknown,
                ..
            })
        )
    })
}

#[derive(Debug, Copy, Clone)]
enum MessageField {
    Msgid,
    Msgstr,
}

impl MessageField {
    fn project<'a>(&self, msgid: &'a str, msgstr: &'a str) -> &'a str {
        match self {
            MessageField::Msgid => msgid,
            MessageField::Msgstr => msgstr,
        }
    }
}

#[derive(Debug)]
struct SourceMap<'a> {
    messages: HashMap<&'a str, Vec<(usize, &'a str, &'a str)>>,
}

impl<'a> SourceMap<'a> {
    /// Construct a map from source paths to links.
    fn new(catalog: &'a Catalog) -> SourceMap<'a> {
        let mut messages = HashMap::<&str, Vec<_>>::new();
        for message in catalog.messages() {
            let path_linenos = message
                .source()
                .split_whitespace()
                .map(|source| parse_source(source).unwrap_or((source, 0)));
            for (path, lineno) in path_linenos {
                messages.entry(path).or_default().push((
                    lineno,
                    message.msgid(),
                    message.msgstr().unwrap_or_default(),
                ));
            }
        }

        for value in messages.values_mut() {
            value.sort();
        }

        SourceMap { messages }
    }

    /// Extract messages for `message`.
    ///
    /// Broken links are resolved using the other messages from the
    /// same path in the source map.
    fn extract_messages(
        &self,
        message: &dyn MessageView,
        field: MessageField,
    ) -> anyhow::Result<Vec<(usize, String)>> {
        // The strategy is to parse the message alone, if possible. If
        // it has a broken link, then we construct a larger document
        // using all other messages with the same path. This way the
        // link should be defined.
        let document = field.project(message.msgid(), message.msgstr()?);
        if !has_broken_link(document) {
            return Ok(extract_document_messages(document));
        }

        // If `parse_source` fails, then `message` has more than one
        // source. We won't attempt to resolve the broken link in that
        // case since it is unclear which link definition to use.
        let path = match parse_source(message.source()) {
            Some((path, _)) => path,
            None => return Ok(extract_document_messages(document)),
        };

        // First, we try constructing a document using other messages
        // from the catalog. Catalogs from pre-0.1.0 included the link
        // definitions.
        //
        // This will have quadratic complexity in case every message
        // from `path` has a "[some text][1]" link which needs to be
        // resolved using a table of link definitions as the bottom.
        // However, in practice, only a few messages will have such a
        // link and the whole thing seems to be fast enough.
        let mut full_document = String::from(document);
        for (_, msgid, msgstr) in &self.messages[path] {
            let msg = field.project(msgid, msgstr);
            if msg == document {
                continue;
            }
            full_document.push_str("\n\n");
            full_document.push_str(msg);
        }

        // Second, we attempt to add the original source file.
        // Catalogs made with version 0.1.0 to 0.2.0 did not include
        // the link definitions at all, so we need to rely on the
        // source data (if we can find it).
        if let Ok(mut file) = File::open(path) {
            full_document.push_str("\n\n");
            let _ = file.read_to_string(&mut full_document);
        }

        let mut messages = extract_document_messages(&full_document);
        // Truncate away the messages from `full_document` which start
        // after `document`.
        let line_count = document.lines().count();
        if let Some(pos) = messages.iter().position(|(lineno, _)| *lineno > line_count) {
            messages.truncate(pos);
        }
        Ok(messages)
    }
}

/// Normalize all entries in the catalog.
///
/// Both the `msgid` and the `msgstr` fields are sent through
/// [`extract_messages`]. The resulting messages are emitted to a new
/// catalog. If the normalization produces different number of
/// messages for the `msgid` and `msgstr` fields, then the result is
/// marked fuzzy. The extra messages are dropped.
pub fn normalize(catalog: Catalog) -> anyhow::Result<Catalog> {
    let source_map = SourceMap::new(&catalog);

    // Accumulate new messages here to avoid constructing a `Catalog`
    // via a partial move from `catalog`.
    let mut new_messages = Vec::new();
    for message in catalog.messages() {
        let new_msgids = source_map.extract_messages(message, MessageField::Msgid)?;
        if new_msgids.is_empty() {
            // Continue if there is nothing to normalize. This can
            // happen if the old `msgid` is something like "<b>Foo"
            // since we no longer extract HTML elements.
            continue;
        }

        let mut new_msgstrs = source_map.extract_messages(message, MessageField::Msgstr)?;
        let mut flags = MessageFlags::new();
        if message.is_fuzzy() || (message.is_translated() && new_msgids.len() != new_msgstrs.len())
        {
            // Keep existing fuzzy flag, or add a new one if we cannot
            // split a translated message cleanly.
            flags.add_flag("fuzzy");
        }

        match new_msgids.len().cmp(&new_msgstrs.len()) {
            std::cmp::Ordering::Less => {
                // Treat left-over translations as separate paragraphs.
                // This makes normalization stable.
                let tail = new_msgstrs[new_msgids.len() - 1..]
                    .iter()
                    .map(|(_, msgstr)| msgstr.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                new_msgstrs.truncate(new_msgids.len() - 1);
                new_msgstrs.push((0, tail));
            }
            std::cmp::Ordering::Greater => {
                // Set missing msgstr entries to "".
                new_msgstrs.resize(new_msgids.len(), (0, String::new()));
            }
            _ => {}
        }

        for ((delta, msgid), (_, msgstr)) in std::iter::zip(new_msgids, new_msgstrs) {
            let new_message = Message::build_singular()
                .with_source(compute_source(message.source(), delta - 1))
                .with_msgid(msgid)
                .with_msgstr(msgstr)
                .with_flags(flags.clone())
                .done();
            new_messages.push(new_message);
        }
    }

    let mut new_catalog = Catalog::new(catalog.metadata);
    for new_message in new_messages {
        match new_catalog.find_message_mut(None, new_message.msgid(), None) {
            Some(mut message) => {
                if !message.is_translated() && new_message.is_translated() {
                    message.set_msgstr(String::from(new_message.msgstr()?))?;
                    // Because we normalize messages like "# Foo" and
                    // "- Foo" to just "Foo", we can end up with
                    // duplicates. In that case, it's important to
                    // preserve the fuzzy flag.
                    if new_message.is_fuzzy() {
                        message.flags_mut().add_flag("fuzzy");
                    }
                }
                message.source_mut().push('\n');
                message.source_mut().push_str(new_message.source());
            }
            None => new_catalog.append_or_update(new_message),
        }
    }

    Ok(new_catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polib::metadata::CatalogMetadata;
    use pretty_assertions::assert_eq;

    // Create a catalog from the translation pairs given.
    fn create_catalog(translations: &[(&str, &str)]) -> Catalog {
        let mut catalog = Catalog::new(CatalogMetadata::new());
        for (idx, (msgid, msgstr)) in translations.iter().enumerate() {
            let message = Message::build_singular()
                .with_source(format!("foo.md:{idx}"))
                .with_msgid(String::from(*msgid))
                .with_msgstr(String::from(*msgstr))
                .done();
            catalog.append_or_update(message);
        }
        catalog
    }

    fn exact<'a>(msgid: &'a str, msgstr: &'a str) -> (bool, &'a str, &'a str) {
        (false, msgid, msgstr)
    }

    fn fuzzy<'a>(msgid: &'a str, msgstr: &'a str) -> (bool, &'a str, &'a str) {
        (true, msgid, msgstr)
    }

    #[track_caller]
    fn assert_normalized_messages_eq(catalog: Catalog, expected_messages: &[(bool, &str, &str)]) {
        let normalized = normalize(catalog).expect("Could not normalize");
        let messages = normalized
            .messages()
            .map(|msg| (msg.is_fuzzy(), msg.msgid(), msg.msgstr().unwrap()))
            .collect::<Vec<(bool, &str, &str)>>();
        assert_eq!(messages, expected_messages);
    }

    #[test]
    fn test_normalize_untranslated() {
        let catalog = create_catalog(&[("foo bar", "")]);
        assert_normalized_messages_eq(catalog, &[exact("foo bar", "")]);
    }

    #[test]
    fn test_normalize_first_wins() {
        // When two or more msgid fields are normalized the same way,
        // we use the first translated entry. The other is dropped.
        let catalog = create_catalog(&[("foo", "FOO 1"), ("# foo", "# FOO 2")]);
        assert_normalized_messages_eq(catalog, &[exact("foo", "FOO 1")]);
    }

    #[test]
    fn test_normalize_early_translation_wins() {
        let catalog = create_catalog(&[("foo", "FOO 1"), ("# foo", "")]);
        assert_normalized_messages_eq(catalog, &[exact("foo", "FOO 1")]);
    }

    #[test]
    fn test_normalize_late_translation_wins() {
        let catalog = create_catalog(&[("foo", ""), ("# foo", "# FOO 2")]);
        assert_normalized_messages_eq(catalog, &[exact("foo", "FOO 2")]);
    }

    #[test]
    fn test_normalize_fuzzy_wins() {
        let mut catalog = create_catalog(&[("foo", ""), ("# foo", "# FOO 2")]);
        // Make the second message fuzzy and check that this is copied
        // to the normalized messages.
        catalog
            .messages_mut()
            .nth(1)
            .unwrap()
            .flags_mut()
            .add_flag("fuzzy");
        assert_normalized_messages_eq(catalog, &[fuzzy("foo", "FOO 2")]);
    }

    #[test]
    fn test_normalize_softbreak() {
        let catalog = create_catalog(&[("foo\nbar", "FOO\nBAR\nBAZ")]);
        assert_normalized_messages_eq(catalog, &[exact("foo bar", "FOO BAR BAZ")]);
    }

    #[test]
    fn test_normalize_inline_link() {
        let catalog = create_catalog(&[(
            "foo [bar](http://example.net/) baz",
            "FOO [BAR](http://example.net/) BAZ",
        )]);
        assert_normalized_messages_eq(
            catalog,
            &[exact(
                "foo [bar](http://example.net/) baz",
                "FOO [BAR](http://example.net/) BAZ",
            )],
        );
    }

    #[test]
    fn test_normalize_reference_link() {
        // Check that we can normalize a reference link when its link
        // definition is in a different message.
        let catalog = create_catalog(&[
            ("Unrelated paragraph before.", "UNRELATED PARAGRAPH BEFORE."),
            (
                "foo [bar][reference-link] baz",
                "FOO [BAR][reference-link] BAZ",
            ),
            ("Unrelated paragraph after.", "UNRELATED PARAGRAPH AFTER."),
            (
                "[reference-link]: http://example.net/\n\
                 [other-link]: http://example.com/",
                "[reference-link]: HTTP://EXAMPLE.NET/\n\
                 [other-link]: HTTP://EXAMPLE.COM/",
            ),
        ]);
        assert_normalized_messages_eq(
            catalog,
            &[
                exact("Unrelated paragraph before.", "UNRELATED PARAGRAPH BEFORE."),
                exact(
                    "foo [bar](http://example.net/) baz",
                    "FOO [BAR](HTTP://EXAMPLE.NET/) BAZ",
                ),
                exact("Unrelated paragraph after.", "UNRELATED PARAGRAPH AFTER."),
            ],
        );
    }

    #[test]
    fn test_normalize_paragraphs() {
        let catalog = create_catalog(&[(
            "foo\n\n\
             bar",
            "FOO\n\n\
             BAR",
        )]);
        assert_normalized_messages_eq(catalog, &[exact("foo", "FOO"), exact("bar", "BAR")]);
    }

    #[test]
    fn test_normalize_fuzzy_paragraphs_too_many() {
        let catalog = create_catalog(&[(
            "foo\n\n\
             bar",
            "FOO\n\n\
             BAR\n\n\
             BAZ",
        )]);
        assert_normalized_messages_eq(catalog, &[fuzzy("foo", "FOO"), fuzzy("bar", "BAR\n\nBAZ")]);
    }

    #[test]
    fn test_normalize_fuzzy_paragraphs_too_few() {
        let catalog = create_catalog(&[(
            "foo\n\n\
             bar\n\n\
             baz",
            "FOO\n\n\
             BAR",
        )]);
        assert_normalized_messages_eq(
            catalog,
            &[fuzzy("foo", "FOO"), fuzzy("bar", "BAR"), fuzzy("baz", "")],
        );
    }

    #[test]
    fn test_normalize_list_items() {
        let catalog = create_catalog(&[(
            "* foo\n\
             * bar",
            "* FOO\n\
             * BAR",
        )]);
        assert_normalized_messages_eq(catalog, &[exact("foo", "FOO"), exact("bar", "BAR")]);
    }

    #[test]
    fn test_normalize_fuzzy_list_items_too_many() {
        let catalog = create_catalog(&[(
            "* foo\n\
             * bar",
            "* FOO\n\
             * BAR\n\
             * BAZ",
        )]);
        assert_normalized_messages_eq(catalog, &[fuzzy("foo", "FOO"), fuzzy("bar", "BAR\n\nBAZ")]);
    }

    #[test]
    fn test_normalize_fuzzy_list_items_too_few() {
        let catalog = create_catalog(&[(
            "* foo\n\
             * bar\n\
             * baz",
            "* FOO\n\
             * BAR",
        )]);
        assert_normalized_messages_eq(
            catalog,
            &[fuzzy("foo", "FOO"), fuzzy("bar", "BAR"), fuzzy("baz", "")],
        );
    }

    #[test]
    fn test_normalize_inline_html() {
        // Inline tags are kept as is.
        let catalog = create_catalog(&[("foo <span>bar</span>", "FOO <span>BAR</span>")]);
        assert_normalized_messages_eq(
            catalog,
            &[exact("foo <span>bar</span>", "FOO <span>BAR</span>")],
        );
    }

    #[test]
    fn test_normalize_block_html() {
        // Block level tags are not translated.
        let catalog = create_catalog(&[(
            "<div class=\"warning\">\n\
            \n\
            foo\n\
            \n\
            </div>",
            "<div class=\"warning\">\n\
            \n\
            FOO\n\
            \n\
            </div>",
        )]);
        assert_normalized_messages_eq(catalog, &[exact("foo", "FOO")]);
    }

    #[test]
    fn test_normalize_disappearing_html() {
        // Normalizing "<b>" results in no messages.
        let catalog = create_catalog(&[("<b>", "FOO")]);
        assert_normalized_messages_eq(catalog, &[]);
    }

    #[test]
    fn test_normalize_code_blocks() {
        let catalog = create_catalog(&[(
            "```rust,editable\n\
             // Example\n\
             foo\n\
             \n\
             * bar\n\
             ```",
            "```rust,editable\n\
             // Beispiel\n\
             FOO\n\
             \n\
             * BAR\n\
             ```",
        )]);
        assert_normalized_messages_eq(catalog, &[exact("// Example\n", "// Beispiel\n")]);
    }

    #[test]
    fn test_normalize_block_quote() {
        let catalog = create_catalog(&[(
            "> foo bar\n\
             > baz",
            "> FOO BAR\n\
             > BAZ",
        )]);
        assert_normalized_messages_eq(catalog, &[exact("foo bar baz", "FOO BAR BAZ")]);
    }

    #[test]
    fn test_normalize_block_quote_with_list() {
        let catalog = create_catalog(&[(
            "> * foo bar\n\
             >   baz\n\
             > * quux",
            "> * FOO BAR\n\
             >   BAZ\n\
             > * QUUX",
        )]);
        assert_normalized_messages_eq(
            catalog,
            &[exact("foo bar baz", "FOO BAR BAZ"), exact("quux", "QUUX")],
        );
    }

    #[test]
    fn test_normalize_table() {
        let catalog = create_catalog(&[(
            "\
            |        | Types       |\n\
            |--------|-------------|\n\
            | Arrays | `[T; N]`    |\n\
            | Tuples | `()`, ...   |",
            "\
            |   | TYPES |\n\
            |---|---|\n\
            | ARRAYS | `[T; N]`  |\n\
            | TUPLES | `()`, ... |",
        )]);
        assert_normalized_messages_eq(
            catalog,
            &[
                exact("Types", "TYPES"),
                exact("Arrays", "ARRAYS"),
                exact("`[T; N]`", "`[T; N]`"),
                exact("Tuples", "TUPLES"),
                exact("`()`, ...", "`()`, ..."),
            ],
        );
    }
}
