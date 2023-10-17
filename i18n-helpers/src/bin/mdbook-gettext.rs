// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! `gettext` for `mdbook`
//!
//! This program works like `gettext`, meaning it will translate
//! strings in your book.
//!
//! The translations come from GNU Gettext `xx.po` files. The PO file
//! is is found under `po` directory based on the `book.language`. For
//! example, `book.langauge` is set to `ko`, then `po/ko.po` is used.
//! You can set `preprocessor.gettext.po-dir` to specify where to find
//! PO files. If the PO file is not found, you'll get the untranslated
//! book.

use anyhow::{anyhow, Context};
use mdbook::book::Book;
use mdbook::preprocess::{CmdPreprocessor, PreprocessorContext};
use mdbook::BookItem;
use mdbook_i18n_helpers::{extract_events, reconstruct_markdown, translate_events};
use polib::catalog::Catalog;
use polib::message::Message;
use polib::po_file;
use pulldown_cmark::Event;
use semver::{Version, VersionReq};
use std::{io, process};

/// Strip formatting from a Markdown string.
///
/// The string can only contain inline text. Formatting such as
/// emphasis and strong emphasis is removed.
///
/// Modelled after `mdbook::summary::stringify_events`.
fn strip_formatting(text: &str) -> String {
    extract_events(text, None)
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Text(text) | Event::Code(text) => Some(text.as_ref()),
            Event::SoftBreak => Some(" "),
            _ => None,
        })
        .collect()
}

fn translate(text: &str, catalog: &Catalog) -> String {
    let events = extract_events(text, None);
    let translated_events = translate_events(&events, catalog);
    let (translated, _) = reconstruct_markdown(&translated_events, None);
    translated
}

/// Update `catalog` with stripped messages from `SUMMARY.md`.
///
/// While it is permissible to include formatting in the `SUMMARY.md`
/// file, `mdbook` will strip it out when rendering the book. It will
/// also strip formatting when sending the book to preprocessors.
///
/// To be able to find the translations for the `SUMMARY.md` file, we
/// append versions of these messages stripped of formatting.
fn add_stripped_summary_translations(catalog: &mut Catalog) {
    let mut stripped_messages = Vec::new();
    for msg in catalog.messages() {
        // The `SUMMARY.md` filename is fixed, but we cannot assume
        // that the file is at `src/SUMMARY.md` since the `src/`
        // directory can be configured.
        if !msg.source().contains("SUMMARY.md") {
            continue;
        }

        let message = Message::build_singular()
            .with_msgid(strip_formatting(msg.msgid()))
            .with_msgstr(strip_formatting(msg.msgstr().unwrap()))
            .done();
        stripped_messages.push(message);
    }

    for msg in stripped_messages {
        catalog.append_or_update(msg);
    }
}

/// Translte an entire book.
fn translate_book(ctx: &PreprocessorContext, mut book: Book) -> anyhow::Result<Book> {
    // Translation is a no-op when the target language is not set
    let language = match &ctx.config.book.language {
        Some(language) => language,
        None => return Ok(book),
    };

    // Find PO file for the target language.
    let cfg = ctx
        .config
        .get_preprocessor("gettext")
        .ok_or_else(|| anyhow!("Could not read preprocessor.gettext configuration"))?;
    let po_dir = cfg.get("po-dir").and_then(|v| v.as_str()).unwrap_or("po");
    let path = ctx.root.join(po_dir).join(format!("{language}.po"));
    // Nothing to do if PO file is missing.
    if !path.exists() {
        return Ok(book);
    }

    let mut catalog = po_file::parse(&path)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("Could not parse {:?} as PO file", path))?;
    add_stripped_summary_translations(&mut catalog);
    book.for_each_mut(|item| match item {
        BookItem::Chapter(ch) => {
            ch.content = translate(&ch.content, &catalog);
            ch.name = translate(&ch.name, &catalog);
        }
        BookItem::Separator => {}
        BookItem::PartTitle(title) => {
            *title = translate(title, &catalog);
        }
    });

    Ok(book)
}

fn preprocess() -> anyhow::Result<()> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;
    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;
    #[allow(clippy::print_stderr)]
    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The gettext preprocessor was built against \
             mdbook version {}, but we're being called from version {}",
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let translated_book = translate_book(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &translated_book)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    if std::env::args().len() == 3 {
        assert_eq!(std::env::args().nth(1).as_deref(), Some("supports"));
        if let Some("xgettext") = std::env::args().nth(2).as_deref() {
            process::exit(1)
        } else {
            // Signal that we support all other renderers.
            process::exit(0);
        }
    }

    preprocess()
}

#[cfg(test)]
mod tests {
    use super::*;
    use polib::message::{Message, MessageMutView};
    use polib::metadata::CatalogMetadata;
    use pretty_assertions::assert_eq;

    fn create_catalog(translations: &[(&str, &str)]) -> Catalog {
        let mut catalog = Catalog::new(CatalogMetadata::new());
        for (msgid, msgstr) in translations {
            let message = Message::build_singular()
                .with_msgid(String::from(*msgid))
                .with_msgstr(String::from(*msgstr))
                .done();
            catalog.append_or_update(message);
        }
        catalog
    }

    #[test]
    fn test_add_stripped_summary_translations() {
        // Add two messages which map to the same stripped message.
        let mut catalog = create_catalog(&[
            ("foo `bar`", "FOO `BAR`"),
            ("**foo** _bar_", "**FOO** _BAR_"),
        ]);
        for (idx, mut msg) in catalog.messages_mut().enumerate() {
            // Set the source to SUMMARY.md to ensure
            // add_stripped_summary_translations will add a stripped
            // version.
            *msg.source_mut() = format!("src/SUMMARY.md:{idx}");
        }
        add_stripped_summary_translations(&mut catalog);

        // We now have two messages, one with and one without
        // formatting. This lets us handle both the TOC and any
        // occurance on the page.
        assert_eq!(
            catalog
                .messages()
                .map(|msg| (msg.source(), msg.msgid(), msg.msgstr().unwrap()))
                .collect::<Vec<_>>(),
            &[
                ("src/SUMMARY.md:0", "foo `bar`", "FOO `BAR`"),
                ("src/SUMMARY.md:1", "**foo** _bar_", "**FOO** _BAR_"),
                ("", "foo bar", "FOO BAR")
            ]
        );
    }

    #[test]
    fn test_translate_single_line() {
        let catalog = create_catalog(&[("foo bar", "FOO BAR")]);
        assert_eq!(translate("foo bar", &catalog), "FOO BAR");
    }

    #[test]
    fn test_translate_single_paragraph() {
        let catalog = create_catalog(&[("foo bar", "FOO BAR")]);
        // The output is normalized so the newline disappears.
        assert_eq!(translate("foo bar\n", &catalog), "FOO BAR");
    }

    #[test]
    fn test_translate_paragraph_with_leading_newlines() {
        let catalog = create_catalog(&[("foo bar", "FOO BAR")]);
        // The output is normalized so the newlines disappear.
        assert_eq!(translate("\n\n\nfoo bar\n", &catalog), "FOO BAR");
    }

    #[test]
    fn test_translate_paragraph_with_trailing_newlines() {
        let catalog = create_catalog(&[("foo bar", "FOO BAR")]);
        // The output is normalized so the newlines disappear.
        assert_eq!(translate("foo bar\n\n\n", &catalog), "FOO BAR");
    }

    #[test]
    fn test_translate_multiple_paragraphs() {
        let catalog = create_catalog(&[("foo bar", "FOO BAR")]);
        assert_eq!(
            translate(
                "first paragraph\n\
                 \n\
                 foo bar\n\
                 \n\
                 last paragraph\n",
                &catalog
            ),
            "first paragraph\n\
             \n\
             FOO BAR\n\
             \n\
             last paragraph"
        );
    }

    #[test]
    fn test_translate_multiple_paragraphs_extra_newlines() {
        // Notice how the translated paragraphs have more lines.
        let catalog = create_catalog(&[
            ("first paragraph", "FIRST TRANSLATED PARAGRAPH"),
            ("last paragraph", "LAST TRANSLATED PARAGRAPH"),
        ]);
        // Paragraph separation is normalized when translating.
        assert_eq!(
            translate(
                "first\n\
                 paragraph\n\
                 \n\
                 \n\
                 last\n\
                 paragraph\n",
                &catalog
            ),
            "FIRST TRANSLATED PARAGRAPH\n\
             \n\
             LAST TRANSLATED PARAGRAPH"
        );
    }

    #[test]
    fn test_translate_code_block() {
        let catalog = create_catalog(&[(
            "```rust,editable\n\
             fn foo() {\n\n    let x = \"hello\";\n\n}\n\
             ```",
            "```rust,editable\n\
             fn FOO() {\n\n    let X = \"guten tag\";\n\n}\n\
             ```",
        )]);
        assert_eq!(
            translate(
                "Text before.\n\
                 \n\
                 \n\
                 ```rust,editable\n\
                 fn foo() {\n\n    let x = \"hello\";\n\n}\n\
                 ```\n\
                 \n\
                 Text after.\n",
                &catalog
            ),
            "Text before.\n\
             \n\
             ```rust,editable\n\
             fn FOO() {\n\n    let X = \"guten tag\";\n\n}\n\
             ```\n\
             \n\
             Text after.",
        );
    }

    #[test]
    fn test_translate_table() {
        let catalog = create_catalog(&[
            ("Types", "TYPES"),
            ("Literals", "LITERALS"),
            ("Arrays", "ARRAYS"),
            ("Tuples", "TUPLES"),
        ]);
        // The alignment is lost when we generate new Markdown.
        assert_eq!(
            translate(
                "\
                |        | Types       | Literals        |\n\
                |--------|-------------|-----------------|\n\
                | Arrays | `[T; N]`    | `[20, 30, 40]`  |\n\
                | Tuples | `()`, ...   | `()`, `('x',)`  |",
                &catalog
            ),
            "\
            ||TYPES|LITERALS|\n\
            |--|-----|--------|\n\
            |ARRAYS|`[T; N]`|`[20, 30, 40]`|\n\
            |TUPLES|`()`, ...|`()`, `('x',)`|",
        );
    }

    #[test]
    fn test_footnote() {
        let catalog = create_catalog(&[
            ("A footnote[^note].", "A FOOTNOTE[^note]."),
            ("More details.", "MORE DETAILS."),
        ]);
        assert_eq!(
            translate("A footnote[^note].\n\n[^note]: More details.", &catalog),
            "A FOOTNOTE[^note].\n\n[^note]: MORE DETAILS."
        );
    }

    #[test]
    fn test_strikethrough() {
        let catalog = create_catalog(&[("~~foo~~", "~~FOO~~")]);
        assert_eq!(translate("~~foo~~", &catalog), "~~FOO~~");
    }

    #[test]
    fn test_tasklists() {
        let catalog = create_catalog(&[("Foo", "FOO"), ("Bar", "BAR")]);
        assert_eq!(
            translate(
                "\
                - [x] Foo\n\
                - [ ] Bar\n\
                ",
                &catalog
            ),
            "\
            - [x] FOO\n\
            - [ ] BAR",
        );
    }

    #[test]
    fn test_heading_attributes() {
        let catalog = create_catalog(&[("Foo", "FOO"), ("Bar", "BAR")]);
        assert_eq!(
            translate("# Foo { #id .foo }", &catalog),
            "# FOO {#id .foo}"
        );
    }
}
