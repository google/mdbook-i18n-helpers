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
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
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
            translate_book(&catalog, &mut book);
        }
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "xgettext"
    }
}
