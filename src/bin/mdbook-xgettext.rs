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

//! `xgettext` for `mdbook`
//!
//! This program works like `xgettext`, meaning it will extract
//! translatable strings from your book. The strings are saved in a
//! GNU Gettext `messages.pot` file in your build directory (typically
//! `po/messages.pot`).

use anyhow::{anyhow, Context};
use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use mdbook_i18n_helpers::extract_msgs;
use polib::catalog::Catalog;
use polib::message::Message;
use polib::metadata::CatalogMetadata;
use std::{fs, io};

fn add_message(catalog: &mut Catalog, msgid: &str, source: &str) {
    let sources = match catalog.find_message(None, msgid, None) {
        Some(msg) => format!("{}\n{}", msg.source(), source),
        None => String::from(source),
    };
    let message = Message::build_singular()
        .with_source(sources)
        .with_msgid(String::from(msgid))
        .done();
    catalog.append_or_update(message);
}

fn create_catalog(ctx: &RenderContext) -> anyhow::Result<Catalog> {
    let mut metadata = CatalogMetadata::new();
    if let Some(title) = &ctx.config.book.title {
        metadata.project_id_version = String::from(title);
    }
    if let Some(lang) = &ctx.config.book.language {
        metadata.language = String::from(lang);
    }
    metadata.mime_version = String::from("1.0");
    metadata.content_type = String::from("text/plain; charset=UTF-8");
    metadata.content_transfer_encoding = String::from("8bit");
    let mut catalog = Catalog::new(metadata);

    // First, add all chapter names and part titles from SUMMARY.md.
    // The book items are in order of the summary, so we can assign
    // correct line numbers for duplicate lines by tracking the index
    // of our last search.
    let summary_path = ctx.config.book.src.join("SUMMARY.md");
    let summary = std::fs::read_to_string(ctx.root.join(&summary_path))
        .with_context(|| anyhow!("Failed to read {}", summary_path.display()))?;
    let mut last_idx = 0;
    for item in ctx.book.iter() {
        let line = match item {
            BookItem::Chapter(chapter) => &chapter.name,
            BookItem::PartTitle(title) => title,
            BookItem::Separator => continue,
        };

        let idx = summary[last_idx..].find(line).ok_or_else(|| {
            anyhow!(
                "Could not find {line:?} in SUMMARY.md after line {} -- \
                 please remove any formatting from SUMMARY.md",
                summary[..last_idx].lines().count()
            )
        })?;
        last_idx += idx;
        let lineno = summary[..last_idx].lines().count();
        let source = format!("{}:{}", summary_path.display(), lineno);
        add_message(&mut catalog, line, &source);
    }

    // Next, we add the chapter contents.
    for item in ctx.book.iter() {
        if let BookItem::Chapter(chapter) = item {
            let path = match &chapter.path {
                Some(path) => ctx.config.book.src.join(path),
                None => continue,
            };
            for msg in extract_msgs(&chapter.content) {
                let source = format!("{}:{}", path.display(), msg.line_number());
                add_message(&mut catalog, msg.text(&chapter.content), &source);
            }
        }
    }

    Ok(catalog)
}

fn main() -> anyhow::Result<()> {
    let ctx = RenderContext::from_json(&mut io::stdin()).context("Parsing stdin")?;
    let cfg = ctx
        .config
        .get_renderer("xgettext")
        .ok_or_else(|| anyhow!("Could not read output.xgettext configuration"))?;
    let path = cfg
        .get("pot-file")
        .ok_or_else(|| anyhow!("Missing output.xgettext.pot-file config value"))?
        .as_str()
        .ok_or_else(|| anyhow!("Expected a string for output.xgettext.pot-file"))?;
    fs::create_dir_all(&ctx.destination)
        .with_context(|| format!("Could not create {}", ctx.destination.display()))?;
    let output_path = ctx.destination.join(path);
    let catalog = create_catalog(&ctx).context("Extracting messages")?;
    polib::po_file::write(&catalog, &output_path)
        .with_context(|| format!("Writing messages to {}", output_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdbook::MDBook;

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
    fn test_create_catalog_defaults() -> anyhow::Result<()> {
        let (ctx, _tmp) =
            create_render_context(&[("book.toml", "[book]"), ("src/SUMMARY.md", "")])?;

        let catalog = create_catalog(&ctx).unwrap();
        assert_eq!(catalog.metadata.project_id_version, "");
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

        let catalog = create_catalog(&ctx).unwrap();
        assert_eq!(catalog.metadata.project_id_version, "My Translatable Book");
        assert_eq!(catalog.metadata.language, "fr");
        Ok(())
    }

    #[test]
    fn test_create_catalog_summary_formatting() -> anyhow::Result<()> {
        // It is an error to include formatting in the summary file:
        // it is stripped by mdbook and we cannot find it later when
        // trying to translate the book.
        let (ctx, _tmp) = create_render_context(&[
            ("book.toml", "[book]"),
            ("src/SUMMARY.md", "- [foo *bar* baz]()"),
        ])?;

        assert!(create_catalog(&ctx).is_err());
        Ok(())
    }

    #[test]
    fn test_create_catalog() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            ("book.toml", "[book]"),
            ("src/SUMMARY.md", "- [The Foo Chapter](foo.md)"),
            (
                "src/foo.md",
                "# How to Foo\n\
                 \n\
                 The first paragraph about Foo.\n\
                 Still the first paragraph.\n",
            ),
        ])?;

        let catalog = create_catalog(&ctx)?;

        for msg in catalog.messages() {
            assert!(!msg.is_translated());
        }

        assert_eq!(
            catalog
                .messages()
                .map(|msg| msg.msgid())
                .collect::<Vec<&str>>(),
            &[
                "The Foo Chapter",
                "# How to Foo",
                "The first paragraph about Foo.\n\
                 Still the first paragraph."
            ]
        );

        Ok(())
    }
}
