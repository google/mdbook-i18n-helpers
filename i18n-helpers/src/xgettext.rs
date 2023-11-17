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

//! This file contains main logic used by the binary `mdbook-xgettext`.

use std::{io, path};

use super::{extract_events, extract_messages, reconstruct_markdown, wrap_sources};
use anyhow::{anyhow, Context};
use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use polib::catalog::Catalog;
use polib::message::Message;
use polib::metadata::CatalogMetadata;
use pulldown_cmark::{Event, Tag};

/// Strip an optional link from a Markdown string.
fn strip_link(text: &str) -> String {
    let events = extract_events(text, None)
        .into_iter()
        .filter_map(|(_, event)| match event {
            Event::Start(Tag::Link(..)) => None,
            Event::End(Tag::Link(..)) => None,
            _ => Some((0, event)),
        })
        .collect::<Vec<_>>();
    let (without_link, _) = reconstruct_markdown(&events, None);
    without_link
}

fn add_message(catalog: &mut Catalog, msgid: &str, source: &str) {
    let sources = match catalog.find_message(None, msgid, None) {
        Some(msg) => wrap_sources(&format!("{}\n{}", msg.source(), source)),
        None => String::from(source),
    };
    let message = Message::build_singular()
        .with_source(sources)
        .with_msgid(String::from(msgid))
        .done();
    catalog.append_or_update(message);
}

/// Build catalog from RenderContext
///
/// # Arguments

/// * `ctx` - RenderContext from mdbook library
/// * `summary_reader` - A closure which reads summary at given path
pub fn create_catalog<F>(ctx: &RenderContext, summary_reader: F) -> anyhow::Result<Catalog>
where
    F: Fn(path::PathBuf) -> io::Result<String>,
{
    let mut metadata = CatalogMetadata::new();
    if let Some(title) = &ctx.config.book.title {
        metadata.project_id_version = String::from(title);
    }
    if let Some(lang) = &ctx.config.book.language {
        metadata.language = String::from(lang);
    }
    let now = chrono::Local::now();
    metadata.pot_creation_date = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    metadata.mime_version = String::from("1.0");
    metadata.content_type = String::from("text/plain; charset=UTF-8");
    metadata.content_transfer_encoding = String::from("8bit");
    let mut catalog = Catalog::new(metadata);

    // First, add all chapter names and part titles from SUMMARY.md.
    let summary_path = ctx.config.book.src.join("SUMMARY.md");
    let summary = summary_reader(ctx.root.join(&summary_path))
        .with_context(|| anyhow!("Failed to read {}", summary_path.display()))?;
    for (lineno, msgid) in extract_messages(&summary) {
        let source = format!("{}:{}", summary_path.display(), lineno);
        // The summary is mostly links like "[Foo *Bar*](foo-bar.md)".
        // We strip away the link to get "Foo *Bar*". The formatting
        // is stripped away by mdbook when it sends the book to
        // mdbook-gettext -- we keep the formatting here in case the
        // same text is used for the page title.
        add_message(&mut catalog, &strip_link(&msgid), &source);
    }

    // Next, we add the chapter contents.
    for item in ctx.book.iter() {
        if let BookItem::Chapter(chapter) = item {
            let path = match &chapter.path {
                Some(path) => ctx.config.book.src.join(path),
                None => continue,
            };
            for (lineno, msgid) in extract_messages(&chapter.content) {
                let source = format!("{}:{}", path.display(), lineno);
                add_message(&mut catalog, &msgid, &source);
            }
        }
    }

    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdbook::MDBook;
    use pretty_assertions::assert_eq;

    fn create_render_context(
        files: &[(&str, &str)],
    ) -> anyhow::Result<(RenderContext, tempfile::TempDir)> {
        let tmpdir = tempfile::tempdir().context("Could not create temporary directory")?;
        std::fs::create_dir(tmpdir.path().join("src"))
            .context("Could not create src/ directory")?;

        for (path, contents) in files {
            std::fs::write(tmpdir.path().join(path), contents)
                .with_context(|| format!("Could not write {path}"))?;
        }

        let mdbook = MDBook::load(tmpdir.path()).context("Could not load book")?;
        let ctx = RenderContext::new(mdbook.root, mdbook.book, mdbook.config, "dest");
        Ok((ctx, tmpdir))
    }

    #[test]
    fn test_strip_link_empty() {
        assert_eq!(strip_link(""), "");
    }

    #[test]
    fn test_strip_link_text() {
        assert_eq!(strip_link("Summary"), "Summary");
    }

    #[test]
    fn test_strip_link_with_formatting() {
        // The formatting is automatically normalized.
        assert_eq!(strip_link("[foo *bar* `baz`](foo.md)"), "foo _bar_ `baz`");
    }

    #[test]
    fn test_create_catalog_defaults() -> anyhow::Result<()> {
        let (ctx, _tmp) =
            create_render_context(&[("book.toml", "[book]"), ("src/SUMMARY.md", "")])?;

        let catalog = create_catalog(&ctx, std::fs::read_to_string).unwrap();
        assert_eq!(catalog.metadata.project_id_version, "");
        assert!(!catalog.metadata.pot_creation_date.is_empty());
        assert!(catalog.metadata.po_revision_date.is_empty());
        assert_eq!(catalog.metadata.language, "en");
        assert_eq!(catalog.metadata.mime_version, "1.0");
        assert_eq!(catalog.metadata.content_type, "text/plain; charset=UTF-8");
        assert_eq!(catalog.metadata.content_transfer_encoding, "8bit");
        Ok(())
    }

    #[test]
    fn test_create_catalog_metadata() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 title = \"My Translatable Book\"\n\
                 language = \"fr\"",
            ),
            ("src/SUMMARY.md", ""),
        ])?;

        let catalog = create_catalog(&ctx, std::fs::read_to_string).unwrap();
        assert_eq!(catalog.metadata.project_id_version, "My Translatable Book");
        assert_eq!(catalog.metadata.language, "fr");
        Ok(())
    }

    #[test]
    fn test_create_catalog_summary_formatting() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            ("book.toml", "[book]"),
            (
                "src/SUMMARY.md",
                "# Summary\n\
                 \n\
                 [Prefix Chapter](prefix.md)\n\
                 \n\
                 # Part Title\n\
                 \n\
                 - [Foo *Bar*](foo.md)\n\
                 \n\
                 ----------\n\
                 \n\
                 - [Baz `Quux`](baz.md)\n\
                 \n\
                 [Suffix Chapter](suffix.md)",
            ),
            // Without this, mdbook would automatically create the
            // files based on the summary above. This would add
            // unnecessary headings below.
            ("src/prefix.md", ""),
            ("src/foo.md", ""),
            ("src/baz.md", ""),
            ("src/suffix.md", ""),
        ])?;

        let catalog = create_catalog(&ctx, std::fs::read_to_string)?;
        assert_eq!(
            catalog
                .messages()
                .map(|msg| msg.msgid())
                .collect::<Vec<&str>>(),
            &[
                "Summary",
                "Prefix Chapter",
                "Part Title",
                "Foo _Bar_",
                "Baz `Quux`",
                "Suffix Chapter",
            ]
        );

        Ok(())
    }

    #[test]
    fn test_create_catalog() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            ("book.toml", "[book]"),
            ("src/SUMMARY.md", "- [The *Foo* Chapter](foo.md)"),
            (
                "src/foo.md",
                "# How to Foo\n\
                 \n\
                 First paragraph.\n\
                 Same paragraph.\n",
            ),
        ])?;

        let catalog = create_catalog(&ctx, std::fs::read_to_string)?;

        for msg in catalog.messages() {
            assert!(!msg.is_translated());
        }

        assert_eq!(
            catalog
                .messages()
                .map(|msg| (msg.source(), msg.msgid()))
                .collect::<Vec<_>>(),
            &[
                ("src/SUMMARY.md:1", "The _Foo_ Chapter"),
                ("src/foo.md:1", "How to Foo"),
                ("src/foo.md:3", "First paragraph. Same paragraph."),
            ]
        );

        Ok(())
    }

    #[test]
    fn test_create_catalog_duplicates() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            ("book.toml", "[book]"),
            ("src/SUMMARY.md", "- [Foo](foo.md)"),
            (
                "src/foo.md",
                "# Foo\n\
                 \n\
                 Foo\n",
            ),
        ])?;

        let catalog = create_catalog(&ctx, std::fs::read_to_string)?;
        assert_eq!(
            catalog
                .messages()
                .map(|msg| (msg.source(), msg.msgid()))
                .collect::<Vec<_>>(),
            &[("src/SUMMARY.md:1 src/foo.md:1 src/foo.md:3", "Foo"),]
        );

        Ok(())
    }
}
