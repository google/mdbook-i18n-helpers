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

use std::collections::HashMap;
use std::{io, path};

use super::{extract_events, extract_messages, reconstruct_markdown, wrap_sources};
use anyhow::{anyhow, Context};
use mdbook::renderer::RenderContext;
use mdbook::{book, BookItem};
use polib::catalog::Catalog;
use polib::message::{Message, MessageMutView, MessageView};
use polib::metadata::CatalogMetadata;
use pulldown_cmark::{Event, Tag, TagEnd};

/// Strip an optional link from a Markdown string.
fn strip_link(text: &str) -> String {
    let events = extract_events(text, None)
        .into_iter()
        .filter_map(|(_, event)| match event {
            Event::Start(Tag::Link { .. }) => None,
            Event::End(TagEnd::Link) => None,
            _ => Some((0, event)),
        })
        .collect::<Vec<_>>();
    let (without_link, _) = reconstruct_markdown(&events, None);
    without_link
}

fn add_message(catalog: &mut Catalog, msgid: &str, source: &str, comment: &str) {
    let sources = match catalog.find_message(None, msgid, None) {
        Some(msg) => format!("{}\n{}", msg.source(), source),
        None => String::from(source),
    };
    let message = Message::build_singular()
        .with_source(sources)
        .with_msgid(String::from(msgid))
        .with_comments(String::from(comment))
        .done();
    catalog.append_or_update(message);
}

/// Build a source line for a catalog message.
///
/// Use `granularity` to round `lineno`:
///
/// - Set `granularity` to `1` if you want no rounding.
/// - Set `granularity` to `0` if you want to line number at all.
/// - Set `granularity` to `n` if you want rounding down to the
///   nearest multiple of `n`. As an example, if you set it to `10`,
///   then you will get sources like `foo.md:1`, `foo.md:10`,
///   `foo.md:20`, etc.
///
/// This can help reduce number of updates to your PO files.
fn build_source<P: AsRef<path::Path>>(path: P, lineno: usize, granularity: usize) -> String {
    let path = path.as_ref();
    match granularity {
        0 => format!("{}", path.display()),
        1 => format!("{}:{}", path.display(), lineno),
        _ => format!(
            "{}:{}",
            path.display(),
            std::cmp::max(1, lineno - (lineno % granularity))
        ),
    }
}

fn dedup_sources(catalog: &mut Catalog) {
    for mut message in catalog.messages_mut() {
        let mut lines: Vec<&str> = message.source().lines().collect();
        lines.dedup();

        let wrapped_source = wrap_sources(&lines.join("\n"));
        *message.source_mut() = wrapped_source;
    }
}

/// Build CatalogMetadata from RenderContext
fn generate_catalog_metadata(ctx: &RenderContext) -> CatalogMetadata {
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
    metadata
}

/// Build catalog from RenderContext
///
/// The `summary_reader` is a closure which should return the
/// `SUMMARY.md` found at the given path.
pub fn create_catalogs<F>(
    ctx: &RenderContext,
    summary_reader: F,
) -> anyhow::Result<HashMap<path::PathBuf, Catalog>>
where
    F: Fn(path::PathBuf) -> io::Result<String>,
{
    let metadata = generate_catalog_metadata(ctx);
    let mut catalog = Catalog::new(metadata);

    // The line number granularity: we default to 1, but it can be
    // overridden as needed.
    let granularity = match ctx
        .config
        .get_renderer("xgettext")
        .and_then(|cfg| cfg.get("granularity"))
    {
        None => 1,
        Some(value) => value
            .as_integer()
            .and_then(|i| (i >= 0).then_some(i as usize))
            .ok_or_else(|| {
                anyhow!("Expected an unsigned integer for output.xgettext.granularity")
            })?,
    };

    // First, add all chapter names and part titles from SUMMARY.md.
    let summary_path = ctx.config.book.src.join("SUMMARY.md");
    let summary = summary_reader(ctx.root.join(&summary_path))
        .with_context(|| anyhow!("Failed to read {}", summary_path.display()))?;
    for (lineno, extracted_msg) in extract_messages(&summary) {
        let msgid = extracted_msg.message;
        let source = build_source(&summary_path, lineno, granularity);
        // The summary is mostly links like "[Foo *Bar*](foo-bar.md)".
        // We strip away the link to get "Foo *Bar*". The formatting
        // is stripped away by mdbook when it sends the book to
        // mdbook-gettext -- we keep the formatting here in case the
        // same text is used for the page title.
        add_message(
            &mut catalog,
            &strip_link(&msgid),
            &source,
            &extracted_msg.comment,
        );
    }

    let mut catalogs = HashMap::new();

    // The depth on which to split the output file. The default is to
    // include all messages into a single POT file (depth == 0).
    // Greater values will split POT files, digging into the
    // sub-chapters within each chapter.
    let depth = match ctx
        .config
        .get_renderer("xgettext")
        .and_then(|cfg| cfg.get("depth"))
    {
        None => 0,
        Some(value) => value
            .as_integer()
            .and_then(|i| (i >= 0).then_some(i as usize))
            .ok_or_else(|| anyhow!("Expected an unsigned integer for output.xgettext.depth"))?,
    };

    // The catalog from the summary data will exist in the single pot
    // file for a depth of 0, will exist in a top-level separate
    // `summary.pot` file for a depth of 1, or exist within in a
    // `summary.pot` file within the default directory for chapters
    // without a corresponding part title.
    let mut current_top_level = "summary".to_owned();
    let mut summary_destination = match depth {
        0 => path::PathBuf::from("messages"),
        1 => path::PathBuf::from("summary"),
        _ => path::PathBuf::from(&current_top_level).join("summary"),
    };
    let _: bool = summary_destination.set_extension("pot");
    catalogs.insert(summary_destination, catalog);

    // Next, we add the chapter contents.
    for item in &ctx.book.sections {
        if let BookItem::PartTitle(title) = item {
            // Iterating through the book in section-order, the
            // PartTitle represents the 'section' that each chapter
            // exists within.
            current_top_level = slug(title);
        } else if let BookItem::Chapter(chapter) = item {
            let path = match &chapter.path {
                Some(path) => ctx.config.book.src.join(path),
                None => continue,
            };
            let directory = match depth {
                0 => path::PathBuf::from("messages"),
                1 => path::PathBuf::from(current_top_level.clone()),
                // The current chapter is already at depth 2, so
                // append the chapter's name for depths greater than
                // 1.
                _ => path::PathBuf::from(current_top_level.clone()).join(slug(&chapter.name)),
            };

            // Add the (destination, catalog) to the map if it doesn't
            // yet exist, so messages can be appended to the catalog.
            let mut destination = directory.clone();
            let _: bool = destination.set_extension("pot");
            let catalog = catalogs
                .entry(destination.clone())
                .or_insert_with(|| Catalog::new(generate_catalog_metadata(ctx)));

            for (lineno, extracted) in extract_messages(&chapter.content) {
                let msgid = extracted.message;
                let source = build_source(&path, lineno, granularity);
                add_message(catalog, &msgid, &source, &extracted.comment);
            }

            // Add the contents for all of the sub-chapters within the
            // current chapter.
            for Chapter {
                content,
                source,
                mut destination,
            } in get_subcontent_for_chapter(chapter, directory, depth, 2)
            {
                let _: bool = destination.set_extension("pot");
                let catalog = catalogs
                    .entry(destination.clone())
                    .or_insert_with(|| Catalog::new(generate_catalog_metadata(ctx)));

                let source = ctx.config.book.src.join(&source);
                for (lineno, extracted) in extract_messages(&content) {
                    let msgid = extracted.message;
                    let source = format!("{}:{}", source.display(), lineno);
                    add_message(catalog, &msgid, &source, &extracted.comment);
                }
            }
        }
    }
    catalogs
        .iter_mut()
        .for_each(|(_key, catalog)| dedup_sources(catalog));
    Ok(catalogs)
}

/// A view into the relevant template information held by
/// `mdbook::book::Chapter` and a location to store the exported polib
/// messages.
struct Chapter {
    /// The chapter's content.
    content: String,
    /// The file where the content is sourced.
    source: path::PathBuf,
    /// The output destination for the polib template.
    destination: path::PathBuf,
}

// A recursive function to crawl a chapter's sub-items and get the
// relevant info to produce a set of po template files.
fn get_subcontent_for_chapter(
    c: &book::Chapter,
    provided_file_path: path::PathBuf,
    provided_depth: usize,
    depth: usize,
) -> Vec<Chapter> {
    if c.sub_items.is_empty() {
        return Vec::new();
    };

    // Iterate through sub-chapters and pull the chapter content,
    // path, and destination to store the template.
    c.sub_items
        .iter()
        .filter_map(|item| {
            let BookItem::Chapter(chapter) = item else {
                return None;
            };
            let (chapter_info, new_path) = match &chapter.path {
                Some(chapter_path) => {
                    // Append the chapter's name to the template's
                    // destination when the depth has not surpassed
                    // the provided value.
                    let destination = if depth < provided_depth {
                        provided_file_path.join(slug(&chapter.name))
                    } else {
                        provided_file_path.clone()
                    };

                    let info = Chapter {
                        content: chapter.content.clone(),
                        source: chapter_path.clone(),
                        destination: destination.clone(),
                    };
                    (Some(info), destination)
                }
                None => (None, provided_file_path.clone()),
            };

            // Recursively call to get sub-chapter contents.
            Some(chapter_info.into_iter().chain(get_subcontent_for_chapter(
                chapter,
                new_path,
                provided_depth,
                depth + 1,
            )))
        })
        .flatten()
        .collect()
}

// Trim a string slice to only contain alphanumeric characters and
// dashes.
fn slug(title: &str) -> String {
    // Specially handle "C++" to format it as "cpp" instead of "c".
    let title = title.to_lowercase().replace("c++", "cpp");
    title
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|&ch| ch == '-' || ch.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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

        // Dynamically create the directories for the listed files.
        // Write the contents to the file in the specified directory.
        for (path, contents) in files {
            let file_path = tmpdir.path().join(path);
            let directory_path = file_path
                .parent()
                .context("File path unexpectedly ended in a root or prefix")?;

            std::fs::create_dir_all(directory_path).context(format!(
                "Could not create directory {}",
                directory_path.display()
            ))?;
            std::fs::write(file_path.clone(), contents)
                .with_context(|| format!("Could not write {}", file_path.display()))?;
        }

        let mdbook = MDBook::load(tmpdir.path()).context("Could not load book")?;
        let ctx = RenderContext::new(mdbook.root, mdbook.book, mdbook.config, "dest");
        Ok((ctx, tmpdir))
    }

    fn default_template_file() -> path::PathBuf {
        path::PathBuf::from("messages.pot")
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
    fn test_build_source_granularity_zero() {
        assert_eq!(build_source("foo.md", 0, 0), "foo.md");
        assert_eq!(build_source("foo.md", 1, 0), "foo.md");
        assert_eq!(build_source("foo.md", 9, 0), "foo.md");
        assert_eq!(build_source("foo.md", 10, 0), "foo.md");
        assert_eq!(build_source("foo.md", 11, 0), "foo.md");
        assert_eq!(build_source("foo.md", 20, 0), "foo.md");
    }

    #[test]
    fn test_build_source_granularity_one() {
        assert_eq!(build_source("foo.md", 0, 1), "foo.md:0");
        assert_eq!(build_source("foo.md", 1, 1), "foo.md:1");
        assert_eq!(build_source("foo.md", 9, 1), "foo.md:9");
        assert_eq!(build_source("foo.md", 10, 1), "foo.md:10");
        assert_eq!(build_source("foo.md", 11, 1), "foo.md:11");
        assert_eq!(build_source("foo.md", 20, 1), "foo.md:20");
    }

    #[test]
    fn test_build_source_granularity_ten() {
        assert_eq!(build_source("foo.md", 0, 10), "foo.md:1");
        assert_eq!(build_source("foo.md", 1, 10), "foo.md:1");
        assert_eq!(build_source("foo.md", 9, 10), "foo.md:1");
        assert_eq!(build_source("foo.md", 10, 10), "foo.md:10");
        assert_eq!(build_source("foo.md", 11, 10), "foo.md:10");
        assert_eq!(build_source("foo.md", 20, 10), "foo.md:20");
    }

    #[test]
    fn test_create_catalog_defaults() -> anyhow::Result<()> {
        let (ctx, _tmp) =
            create_render_context(&[("book.toml", "[book]"), ("src/SUMMARY.md", "")])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string).unwrap();
        let catalog = &catalogs[&default_template_file()];
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

        // Using a depth of 0 to include all messages in a single
        // template file.
        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;
        let catalog = &catalogs[&default_template_file()];
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

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;
        let catalog = &catalogs[&default_template_file()];
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
    fn test_create_catalogs() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            ("book.toml", "[book]"),
            ("src/SUMMARY.md", "- [The *Foo* Chapter](foo.md)"),
            (
                "src/foo.md",
                "# How to Foo\n\
                 \n\
                 First paragraph.\n\
                 Same paragraph.\n\
                 \n\
                 [Link](https://example.com)\n",
            ),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;
        let catalog = &catalogs[&default_template_file()];

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
                ("src/foo.md:6", "[Link](https://example.com)"),
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

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;
        let catalog = &catalogs[&default_template_file()];
        assert_eq!(
            catalog
                .messages()
                .map(|msg| (msg.source(), msg.msgid()))
                .collect::<Vec<_>>(),
            &[("src/SUMMARY.md:1 src/foo.md:1 src/foo.md:3", "Foo"),]
        );

        Ok(())
    }

    #[test]
    fn test_create_catalog_lineno_granularity() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 [output.xgettext]\n\
                 granularity = 5",
            ),
            ("src/SUMMARY.md", "- [Foo](foo.md)"),
            (
                "src/foo.md",
                "- Line 1\n\
                 \n\
                 - Line 3\n\
                 \n\
                 - Line 5\n\
                 \n\
                 - Line 7\n\
                 ",
            ),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;
        let catalog = &catalogs[&default_template_file()];
        assert_eq!(
            catalog
                .messages()
                .map(|msg| (msg.source(), msg.msgid()))
                .collect::<Vec<_>>(),
            &[
                ("src/SUMMARY.md:1", "Foo"),
                ("src/foo.md:1", "Line 1"),
                ("src/foo.md:1", "Line 3"),
                ("src/foo.md:5", "Line 5"),
                ("src/foo.md:5", "Line 7"),
            ]
        );

        Ok(())
    }

    #[test]
    fn test_create_catalog_lineno_granularity_duplicates() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 [output.xgettext]\n\
                 granularity = 3",
            ),
            ("src/SUMMARY.md", "- [Foo](foo.md)"),
            (
                "src/foo.md",
                "Bar\n\
                 \n\
                 Bar\n\
                 \n\
                 Bar\n",
            ),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;
        let catalog = &catalogs[&default_template_file()];
        assert_eq!(
            catalog
                .messages()
                .map(|msg| (msg.source(), msg.msgid()))
                .collect::<Vec<_>>(),
            &[
                ("src/SUMMARY.md:1", "Foo"),
                ("src/foo.md:1 src/foo.md:3", "Bar"),
            ]
        );

        Ok(())
    }

    #[test]
    fn test_create_catalog_nested_directories() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            ("book.toml", "[book]"),
            (
                "src/SUMMARY.md",
                "- [The Foo Chapter](foo.md)\n\
                \t- [The Bar Section](foo/bar.md)\n\
                \t\t- [The Baz Subsection](foo/bar/baz.md)",
            ),
            (
                "src/foo.md",
                "# How to Foo\n\
                 \n\
                 The first paragraph about Foo.\n",
            ),
            (
                "src/foo/bar.md",
                "# How to Bar\n\
                 \n\
                 The first paragraph about Bar.\n",
            ),
            (
                "src/foo/bar/baz.md",
                "# How to Baz\n\
                 \n\
                 The first paragraph about Baz.\n",
            ),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;
        let catalog = &catalogs[&default_template_file()];

        for msg in catalog.messages() {
            assert!(!msg.is_translated());
        }

        let expected_message_tuples = vec![
            ("src/SUMMARY.md:1", "The Foo Chapter"),
            ("src/SUMMARY.md:2", "The Bar Section"),
            ("src/SUMMARY.md:3", "The Baz Subsection"),
            ("src/foo.md:1", "How to Foo"),
            ("src/foo.md:3", "The first paragraph about Foo."),
            ("src/foo/bar.md:1", "How to Bar"),
            ("src/foo/bar.md:3", "The first paragraph about Bar."),
            ("src/foo/bar/baz.md:1", "How to Baz"),
            ("src/foo/bar/baz.md:3", "The first paragraph about Baz."),
        ];

        let message_tuples = catalog
            .messages()
            .map(|msg| (msg.source(), msg.msgid()))
            .collect::<Vec<(&str, &str)>>();

        assert_eq!(expected_message_tuples, message_tuples);

        Ok(())
    }

    #[test]
    fn test_split_catalog_nested_directories_depth_1() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 [output.xgettext]\n\
                 depth = 1",
            ),
            (
                "src/SUMMARY.md",
                "# Summary\n\n\
                 - [Intro](index.md)\n\
                 # Foo\n\n\
                 - [The Foo Chapter](foo.md)\n\
                 \t- [The Bar Section](foo/bar.md)\n\
                 \t\t- [The Baz Subsection](foo/bar/baz.md)\n\
                 - [Foo Exercises](exercises/foo.md)",
            ),
            ("src/index.md", "# Intro to X"),
            ("src/foo.md", "# How to Foo"),
            ("src/foo/bar.md", "# How to Bar"),
            ("src/foo/bar/baz.md", "# How to Baz"),
            ("src/exercises/foo.md", "# Exercises on Foo"),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;

        let expected_message_tuples = HashMap::from([
            (
                path::PathBuf::from("summary.pot"),
                vec![
                    "src/SUMMARY.md:1",
                    "src/SUMMARY.md:3",
                    "src/SUMMARY.md:4",
                    "src/SUMMARY.md:6",
                    "src/SUMMARY.md:7",
                    "src/SUMMARY.md:8",
                    "src/SUMMARY.md:9",
                    "src/index.md:1",
                ],
            ),
            (
                path::PathBuf::from("foo.pot"),
                vec![
                    "src/foo.md:1",
                    "src/foo/bar.md:1",
                    "src/foo/bar/baz.md:1",
                    "src/exercises/foo.md:1",
                ],
            ),
        ]);

        assert_eq!(expected_message_tuples.keys().len(), catalogs.len());
        for (file_path, catalog) in catalogs {
            let expected_msgids = &expected_message_tuples[&file_path];
            assert_eq!(
                &catalog
                    .messages()
                    .map(|msg| msg.source())
                    .collect::<Vec<_>>(),
                expected_msgids
            );
        }

        Ok(())
    }

    #[test]
    fn test_split_catalog_nested_directories_depth_2() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 [output.xgettext]\n\
                 depth = 2",
            ),
            (
                "src/SUMMARY.md",
                "# Summary\n\n\
                 - [Intro](index.md)\n\
                 # Foo\n\n\
                 - [The Foo Chapter](foo.md)\n\
                 \t- [The Bar Section](foo/bar.md)\n\
                 \t\t- [The Baz Subsection](foo/bar/baz.md)\n\
                 - [Foo Exercises](exercises/foo.md)",
            ),
            ("src/index.md", "# Intro to X"),
            ("src/foo.md", "# How to Foo"),
            ("src/foo/bar.md", "# How to Bar"),
            ("src/foo/bar/baz.md", "# How to Baz"),
            ("src/exercises/foo.md", "# Exercises on Foo"),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;

        let expected_message_tuples = HashMap::from([
            (
                path::PathBuf::from("summary/summary.pot"),
                vec![
                    "src/SUMMARY.md:1",
                    "src/SUMMARY.md:3",
                    "src/SUMMARY.md:4",
                    "src/SUMMARY.md:6",
                    "src/SUMMARY.md:7",
                    "src/SUMMARY.md:8",
                    "src/SUMMARY.md:9",
                ],
            ),
            (
                path::PathBuf::from("summary/intro.pot"),
                vec!["src/index.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter.pot"),
                vec!["src/foo.md:1", "src/foo/bar.md:1", "src/foo/bar/baz.md:1"],
            ),
            (
                path::PathBuf::from("foo/foo-exercises.pot"),
                vec!["src/exercises/foo.md:1"],
            ),
        ]);

        assert_eq!(expected_message_tuples.keys().len(), catalogs.len());
        for (file_path, catalog) in catalogs {
            let expected_msgids = &expected_message_tuples[&file_path];
            assert_eq!(
                &catalog
                    .messages()
                    .map(|msg| msg.source())
                    .collect::<Vec<_>>(),
                expected_msgids
            );
        }

        Ok(())
    }

    #[test]
    fn test_split_catalog_nested_directories_depth_3() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 [output.xgettext]\n\
                 depth = 3",
            ),
            (
                "src/SUMMARY.md",
                "# Summary\n\n\
                 - [Intro](index.md)\n\
                 # Foo\n\n\
                 - [The Foo Chapter](foo.md)\n\
                 \t- [The Bar Section](foo/bar.md)\n\
                 \t\t- [The Baz Subsection](foo/bar/baz.md)\n\
                 - [Foo Exercises](exercises/foo.md)",
            ),
            ("src/index.md", "# Intro to X"),
            ("src/foo.md", "# How to Foo"),
            ("src/foo/bar.md", "# How to Bar"),
            ("src/foo/bar/baz.md", "# How to Baz"),
            ("src/exercises/foo.md", "# Exercises on Foo"),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;

        let expected_message_tuples = HashMap::from([
            (
                path::PathBuf::from("summary/summary.pot"),
                vec![
                    "src/SUMMARY.md:1",
                    "src/SUMMARY.md:3",
                    "src/SUMMARY.md:4",
                    "src/SUMMARY.md:6",
                    "src/SUMMARY.md:7",
                    "src/SUMMARY.md:8",
                    "src/SUMMARY.md:9",
                ],
            ),
            (
                path::PathBuf::from("summary/intro.pot"),
                vec!["src/index.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter.pot"),
                vec!["src/foo.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter/the-bar-section.pot"),
                vec!["src/foo/bar.md:1", "src/foo/bar/baz.md:1"],
            ),
            (
                path::PathBuf::from("foo/foo-exercises.pot"),
                vec!["src/exercises/foo.md:1"],
            ),
        ]);

        assert_eq!(expected_message_tuples.keys().len(), catalogs.len());
        for (file_path, catalog) in catalogs {
            let expected_msgids = &expected_message_tuples[&file_path];
            assert_eq!(
                &catalog
                    .messages()
                    .map(|msg| msg.source())
                    .collect::<Vec<_>>(),
                expected_msgids
            );
        }

        Ok(())
    }

    #[test]
    fn test_split_catalog_nested_directories_depth_4() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 [output.xgettext]\n\
                 depth = 4",
            ),
            (
                "src/SUMMARY.md",
                "# Summary\n\n\
                 - [Intro](index.md)\n\
                 # Foo\n\n\
                 - [The Foo Chapter](foo.md)\n\
                 \t- [The Bar Section](foo/bar.md)\n\
                 \t\t- [The Baz Subsection](foo/bar/baz.md)\n\
                 - [Foo Exercises](exercises/foo.md)",
            ),
            ("src/index.md", "# Intro to X"),
            ("src/foo.md", "# How to Foo"),
            ("src/foo/bar.md", "# How to Bar"),
            ("src/foo/bar/baz.md", "# How to Baz"),
            ("src/exercises/foo.md", "# Exercises on Foo"),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;

        let expected_message_tuples = HashMap::from([
            (
                path::PathBuf::from("summary/summary.pot"),
                vec![
                    "src/SUMMARY.md:1",
                    "src/SUMMARY.md:3",
                    "src/SUMMARY.md:4",
                    "src/SUMMARY.md:6",
                    "src/SUMMARY.md:7",
                    "src/SUMMARY.md:8",
                    "src/SUMMARY.md:9",
                ],
            ),
            (
                path::PathBuf::from("summary/intro.pot"),
                vec!["src/index.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter.pot"),
                vec!["src/foo.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter/the-bar-section.pot"),
                vec!["src/foo/bar.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter/the-bar-section/the-baz-subsection.pot"),
                vec!["src/foo/bar/baz.md:1"],
            ),
            (
                path::PathBuf::from("foo/foo-exercises.pot"),
                vec!["src/exercises/foo.md:1"],
            ),
        ]);

        assert_eq!(expected_message_tuples.keys().len(), catalogs.len());
        for (file_path, catalog) in catalogs {
            let expected_msgids = &expected_message_tuples[&file_path];
            assert_eq!(
                &catalog
                    .messages()
                    .map(|msg| msg.source())
                    .collect::<Vec<_>>(),
                expected_msgids
            );
        }

        Ok(())
    }

    // The output is expected to be the same as the above test, there
    // should be no difference if the split depth is an arbitrarily
    // large number.
    #[test]
    fn test_split_catalog_nested_directories_depth_greater_than_necessary() -> anyhow::Result<()> {
        let (ctx, _tmp) = create_render_context(&[
            (
                "book.toml",
                "[book]\n\
                 [output.xgettext]\n\
                 depth = 100",
            ),
            (
                "src/SUMMARY.md",
                "# Summary\n\n\
                 - [Intro](index.md)\n\
                 # Foo\n\n\
                 - [The Foo Chapter](foo.md)\n\
                 \t- [The Bar Section](foo/bar.md)\n\
                 \t\t- [The Baz Subsection](foo/bar/baz.md)\n\
                 - [Foo Exercises](exercises/foo.md)",
            ),
            ("src/index.md", "# Intro to X"),
            ("src/foo.md", "# How to Foo"),
            ("src/foo/bar.md", "# How to Bar"),
            ("src/foo/bar/baz.md", "# How to Baz"),
            ("src/exercises/foo.md", "# Exercises on Foo"),
        ])?;

        let catalogs = create_catalogs(&ctx, std::fs::read_to_string)?;

        let expected_message_tuples = HashMap::from([
            (
                path::PathBuf::from("summary/summary.pot"),
                vec![
                    "src/SUMMARY.md:1",
                    "src/SUMMARY.md:3",
                    "src/SUMMARY.md:4",
                    "src/SUMMARY.md:6",
                    "src/SUMMARY.md:7",
                    "src/SUMMARY.md:8",
                    "src/SUMMARY.md:9",
                ],
            ),
            (
                path::PathBuf::from("summary/intro.pot"),
                vec!["src/index.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter.pot"),
                vec!["src/foo.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter/the-bar-section.pot"),
                vec!["src/foo/bar.md:1"],
            ),
            (
                path::PathBuf::from("foo/the-foo-chapter/the-bar-section/the-baz-subsection.pot"),
                vec!["src/foo/bar/baz.md:1"],
            ),
            (
                path::PathBuf::from("foo/foo-exercises.pot"),
                vec!["src/exercises/foo.md:1"],
            ),
        ]);

        assert_eq!(expected_message_tuples.keys().len(), catalogs.len());
        for (file_path, catalog) in catalogs {
            let expected_msgids = &expected_message_tuples[&file_path];
            assert_eq!(
                &catalog
                    .messages()
                    .map(|msg| msg.source())
                    .collect::<Vec<_>>(),
                expected_msgids
            );
        }

        Ok(())
    }
}
