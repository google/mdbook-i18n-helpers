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

use crate::gettext::{add_stripped_summary_translations, translate_book};
use anyhow::{anyhow, Context};
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use polib::catalog::Catalog;
use polib::po_file;
use std::path::PathBuf;

/// Check whether the book should be transalted.
///
/// The book should be translated if:
/// * `book.language` is defined in mdbook config
/// * Corresponding {language}.po defined
fn should_translate(ctx: &PreprocessorContext) -> bool {
    // Translation is a no-op when the target language is not set
    if ctx.config.book.language.is_none() {
        return false;
    }

    // Nothing to do if PO file is missing.
    get_catalog_path(ctx)
        .map(|path| path.try_exists().unwrap_or(false))
        .unwrap_or(false)
}

/// Compute the path of the Catalog file.
fn get_catalog_path(ctx: &PreprocessorContext) -> anyhow::Result<PathBuf> {
    let language = ctx
        .config
        .book
        .language
        .as_ref()
        .ok_or_else(|| anyhow!("Language is not provided"))?;

    let po_dir: String = ctx
        .config
        .get("preprocessor.gettext.po-dir")?
        .unwrap_or_else(|| "po".to_string());
    Ok(ctx.root.join(po_dir).join(format!("{language}.po")))
}

/// Load the catalog with translation strings.
fn load_catalog(ctx: &PreprocessorContext) -> anyhow::Result<Catalog> {
    let path = get_catalog_path(ctx)?;

    let catalog = po_file::parse(&path)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("Could not parse {path:?} as PO file"))?;

    Ok(catalog)
}

/// Preprocessor for gettext
pub struct Gettext;

impl Preprocessor for Gettext {
    fn name(&self) -> &str {
        "gettext"
    }

    fn run(
        &self,
        ctx: &PreprocessorContext,
        mut book: mdbook_preprocessor::book::Book,
    ) -> anyhow::Result<mdbook_preprocessor::book::Book> {
        if should_translate(ctx) {
            let mut catalog = load_catalog(ctx)?;
            add_stripped_summary_translations(&mut catalog);
            translate_book(&catalog, &mut book)?;
        }
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> mdbook_preprocessor::errors::Result<bool> {
        Ok(renderer != "xgettext")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdbook_preprocessor::book::{Book, BookItem, Chapter};
    use mdbook_preprocessor::config::Config;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const DANISH_PO: &str = r#"
msgid ""
msgstr ""
"Language: da\n"
"Content-Type: text/plain; charset=UTF-8\n"

msgid "Hello"
msgstr "Hej"
"#;

    /// Create a context for a book rooted in a fresh temporary directory.
    fn create_context(book_toml: &str) -> (PreprocessorContext, tempfile::TempDir) {
        let tmpdir = tempfile::tempdir().unwrap();
        let config = Config::from_str(book_toml).unwrap();
        let ctx = PreprocessorContext::new(tmpdir.path().to_path_buf(), config, "html".into());
        (ctx, tmpdir)
    }

    fn write_po_file(ctx: &PreprocessorContext, relative_path: &str, contents: &str) {
        let path = ctx.root.join(relative_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn create_book(content: &str) -> Book {
        let mut book = Book::new();
        book.push_item(BookItem::Chapter(Chapter::new(
            "Hello",
            content.into(),
            PathBuf::from("hello.md"),
            Vec::new(),
        )));
        book
    }

    fn chapter_content(book: &Book) -> &str {
        match book.items.first() {
            Some(BookItem::Chapter(chapter)) => &chapter.content,
            item => panic!("expected a chapter, got {item:?}"),
        }
    }

    #[test]
    fn test_name() {
        assert_eq!(Gettext.name(), "gettext");
    }

    #[test]
    fn test_supports_renderer() {
        assert!(Gettext.supports_renderer("html").unwrap());
        assert!(Gettext.supports_renderer("markdown").unwrap());
        // The xgettext renderer extracts the untranslated text.
        assert!(!Gettext.supports_renderer("xgettext").unwrap());
    }

    #[test]
    fn test_get_catalog_path_default_po_dir() {
        let (ctx, tmpdir) = create_context("[book]\nlanguage = \"da\"");
        assert_eq!(
            get_catalog_path(&ctx).unwrap(),
            tmpdir.path().join("po").join("da.po")
        );
    }

    #[test]
    fn test_get_catalog_path_custom_po_dir() {
        let (ctx, tmpdir) = create_context(
            "[book]\nlanguage = \"da\"\n\n[preprocessor.gettext]\npo-dir = \"translations\"",
        );
        assert_eq!(
            get_catalog_path(&ctx).unwrap(),
            tmpdir.path().join("translations").join("da.po")
        );
    }

    #[test]
    fn test_get_catalog_path_without_language() {
        let (mut ctx, _tmpdir) = create_context("");
        // mdbook defaults the language to "en", so clear it explicitly.
        ctx.config.book.language = None;
        assert!(get_catalog_path(&ctx).is_err());
    }

    #[test]
    fn test_should_translate_without_language() {
        let (mut ctx, _tmpdir) = create_context("");
        ctx.config.book.language = None;
        // A PO file alone is not enough, the language must also be set.
        write_po_file(&ctx, "po/en.po", DANISH_PO);
        assert!(!should_translate(&ctx));
    }

    #[test]
    fn test_should_translate_without_po_file() {
        let (ctx, _tmpdir) = create_context("[book]\nlanguage = \"da\"");
        assert!(!should_translate(&ctx));
    }

    #[test]
    fn test_should_translate_with_po_file() {
        let (ctx, _tmpdir) = create_context("[book]\nlanguage = \"da\"");
        write_po_file(&ctx, "po/da.po", DANISH_PO);
        assert!(should_translate(&ctx));
    }

    #[test]
    fn test_run_translates_book() {
        let (ctx, _tmpdir) = create_context("[book]\nlanguage = \"da\"");
        write_po_file(&ctx, "po/da.po", DANISH_PO);
        let book = Gettext.run(&ctx, create_book("Hello")).unwrap();
        assert_eq!(chapter_content(&book), "Hej");
    }

    #[test]
    fn test_run_with_custom_po_dir() {
        let (ctx, _tmpdir) = create_context(
            "[book]\nlanguage = \"da\"\n\n[preprocessor.gettext]\npo-dir = \"translations\"",
        );
        write_po_file(&ctx, "translations/da.po", DANISH_PO);
        let book = Gettext.run(&ctx, create_book("Hello")).unwrap();
        assert_eq!(chapter_content(&book), "Hej");
    }

    #[test]
    fn test_run_without_po_file_leaves_book_unchanged() {
        let (ctx, _tmpdir) = create_context("[book]\nlanguage = \"da\"");
        let book = Gettext.run(&ctx, create_book("Hello")).unwrap();
        assert_eq!(chapter_content(&book), "Hello");
    }

    #[test]
    fn test_run_with_invalid_po_file() {
        let (ctx, _tmpdir) = create_context("[book]\nlanguage = \"da\"");
        write_po_file(&ctx, "po/da.po", "msgid \"Hello\"\nmsgstr \"unterminated");
        assert!(Gettext.run(&ctx, create_book("Hello")).is_err());
    }
}
