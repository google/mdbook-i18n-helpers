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
use mdbook_i18n_helpers::{extract_events, group_events, reconstruct_markdown, Group};
use polib::catalog::Catalog;
use polib::po_file;
use semver::{Version, VersionReq};
use std::{io, process};

fn translate(text: &str, catalog: &Catalog) -> String {
    let mut translated_events = Vec::new();
    let events = extract_events(text, None);
    let mut state = None;

    for group in group_events(&events) {
        match group {
            Group::Translate(events) => {
                // Reconstruct the message.
                let (msgid, new_state) = reconstruct_markdown(events, state.clone());
                let translated = catalog
                    .find_message(None, &msgid, None)
                    .filter(|msg| !msg.flags().is_fuzzy())
                    .and_then(|msg| msg.msgstr().ok())
                    .filter(|msgstr| !msgstr.is_empty());
                // Generate new events or reuse old events.
                match translated {
                    Some(msgstr) => translated_events.extend(extract_events(msgstr, state)),
                    None => translated_events.extend_from_slice(events),
                }
                // Advance the state.
                state = Some(new_state);
            }
            Group::Skip(events) => {
                // Copy the events unchanged to the output.
                translated_events.extend_from_slice(events);
                // Advance the state.
                let (_, new_state) = reconstruct_markdown(events, state);
                state = Some(new_state);
            }
        }
    }

    let (translated, _) = reconstruct_markdown(&translated_events, None);
    translated
}

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

    let catalog = po_file::parse(&path)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("Could not parse {:?} as PO file", path))?;
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
    use polib::message::Message;
    use polib::metadata::CatalogMetadata;

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
            (
                "first\n\
                 paragraph",
                "FIRST\n\
                 TRANSLATED\n\
                 PARAGRAPH",
            ),
            (
                "last\n\
                 paragraph",
                "LAST\n\
                 TRANSLATED\n\
                 PARAGRAPH",
            ),
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
            "FIRST\n\
             TRANSLATED\n\
             PARAGRAPH\n\
             \n\
             LAST\n\
             TRANSLATED\n\
             PARAGRAPH"
        );
    }

    #[test]
    fn test_translate_code_block() {
        let catalog = create_catalog(&[(
            "fn foo() {\n\n    let x = 10;\n\n}\n",
            "fn FOO() {\n\n    let X = 10;\n\n}\n",
        )]);
        assert_eq!(
            translate(
                "Text before.\n\
                 \n\
                 \n\
                 ```rust,editable\n\
                 fn foo() {\n\n    let x = 10;\n\n}\n\
                 ```\n\
                 \n\
                 Text after.\n",
                &catalog
            ),
            "Text before.\n\
             \n\
             ```rust,editable\n\
             fn FOO() {\n\n    let X = 10;\n\n}\n\
             ```\n\
             \n\
             Text after.",
        );
    }
}
