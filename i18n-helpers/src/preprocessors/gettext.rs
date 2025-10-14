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

use crate::gettext::{add_stripped_summary_translations, translate, translate_book};
use anyhow::{anyhow, Context};
use mdbook::book::{Book, BookItem};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use polib::catalog::Catalog;
use polib::po_file;
use std::path::PathBuf;

#[derive(Debug, Default, PartialEq, Eq)]
struct MetadataTranslations {
    title: Option<String>,
    description: Option<String>,
}

fn metadata_translations(
    title: Option<&str>,
    description: Option<&str>,
    catalog: &Catalog,
) -> anyhow::Result<MetadataTranslations> {
    let mut translations = MetadataTranslations::default();

    if let Some(title) = title {
        let translated = translate(title, catalog)?;
        if translated.trim() != title.trim() {
            translations.title = Some(translated);
        }
    }

    if let Some(description) = description {
        let translated = translate(description, catalog)?;
        if translated.trim() != description.trim() {
            translations.description = Some(translated);
        }
    }

    Ok(translations)
}

fn metadata_script(translations: &MetadataTranslations) -> Option<String> {
    if translations.title.is_none() && translations.description.is_none() {
        return None;
    }

    // Serialize eagerly so we fail early if escaping fails for some reason.
    let title_json = translations
        .title
        .as_ref()
        .map(|t| serde_json::to_string(t))
        .transpose()
        .ok()?;
    let description_json = translations
        .description
        .as_ref()
        .map(|d| serde_json::to_string(d))
        .transpose()
        .ok()?;

    let mut script =
        String::from("\n\n<script>window.addEventListener('DOMContentLoaded', function () {\n");
    if let Some(title) = title_json {
        script.push_str("  const title = ");
        script.push_str(&title);
        script.push_str(
            ";\n  if (title) {\n    document.title = title;\n    for (const el of document.querySelectorAll('.menu-title, .mobile-nav__title')) {\n      el.textContent = title;\n    }\n  }\n",
        );
    }

    if let Some(description) = description_json {
        script.push_str("  const description = ");
        script.push_str(&description);
        script.push_str(
            ";\n  if (description) {\n    const meta = document.querySelector('meta[name=\"description\"]');\n    if (meta) {\n      meta.setAttribute('content', description);\n    }\n  }\n",
        );
    }

    script.push_str("});</script>\n");
    Some(script)
}

fn inject_metadata_script(book: &mut Book, translations: &MetadataTranslations) {
    if let Some(script) = metadata_script(translations) {
        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item {
                ch.content.push_str(&script);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdbook::book::{Book, BookItem, Chapter};
    use polib::message::Message;
    use polib::metadata::CatalogMetadata;

    fn catalog_with(entries: &[(&str, &str)]) -> Catalog {
        let mut catalog = Catalog::new(CatalogMetadata::new());
        for (msgid, msgstr) in entries {
            let message = Message::build_singular()
                .with_msgid((*msgid).to_string())
                .with_msgstr((*msgstr).to_string())
                .done();
            catalog.append_or_update(message);
        }
        catalog
    }

    #[test]
    fn injects_script_when_metadata_translated() {
        let catalog = catalog_with(&[
            ("Original Title", "Titre traduit"),
            ("Original description", "Description traduite"),
        ]);

        let translations = metadata_translations(
            Some("Original Title"),
            Some("Original description"),
            &catalog,
        )
        .unwrap();
        assert_eq!(
            translations,
            MetadataTranslations {
                title: Some("Titre traduit".into()),
                description: Some("Description traduite".into()),
            }
        );

        let mut book = Book::new();
        book.push_item(BookItem::Chapter(Chapter::new(
            "Chapter",
            "# Heading".into(),
            "chapter.md",
            vec![],
        )));

        inject_metadata_script(&mut book, &translations);

        let mut contents = Vec::new();
        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item {
                contents.push(ch.content.clone());
            }
        });

        assert!(contents[0].contains("Titre traduit"));
        assert!(contents[0].contains("Description traduite"));
        assert!(contents[0].contains("window.addEventListener"));
    }

    #[test]
    fn skips_script_when_no_translation() {
        let catalog = catalog_with(&[("Same Title", "Same Title"), ("Same description", "")]);

        let translations =
            metadata_translations(Some("Same Title"), Some("Same description"), &catalog).unwrap();
        assert_eq!(translations, MetadataTranslations::default());

        let mut book = Book::new();
        book.push_item(BookItem::Chapter(Chapter::new(
            "Chapter",
            "Content".into(),
            "chapter.md",
            vec![],
        )));

        inject_metadata_script(&mut book, &translations);

        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item {
                assert!(!ch.content.contains("</script>"));
            }
        });
    }
}

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

    let cfg = ctx
        .config
        .get_preprocessor("gettext")
        .ok_or_else(|| anyhow!("Could not read preprocessor.gettext configuration"))?;
    let po_dir = cfg.get("po-dir").and_then(|v| v.as_str()).unwrap_or("po");
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
        mut book: mdbook::book::Book,
    ) -> anyhow::Result<mdbook::book::Book> {
        if should_translate(ctx) {
            let mut catalog = load_catalog(ctx)?;
            add_stripped_summary_translations(&mut catalog);
            let metadata = metadata_translations(
                ctx.config.book.title.as_deref(),
                ctx.config.book.description.as_deref(),
                &catalog,
            )?;
            translate_book(&catalog, &mut book)?;
            inject_metadata_script(&mut book, &metadata);
        }
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "xgettext"
    }
}
