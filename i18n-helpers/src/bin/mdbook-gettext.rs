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
use mdbook::preprocess::{CmdPreprocessor, PreprocessorContext};
use mdbook_i18n_helpers::gettext::{add_stripped_summary_translations, translate_book};
use polib::catalog::Catalog;
use polib::po_file;
use semver::{Version, VersionReq};
use std::path::PathBuf;
use std::{io, process};

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

/// Execute main logic by this mdbook preprocessor.
fn preprocess() -> anyhow::Result<()> {
    let (ctx, mut book) = CmdPreprocessor::parse_input(io::stdin())?;
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

    if should_translate(&ctx) {
        let mut catalog = load_catalog(&ctx)?;
        add_stripped_summary_translations(&mut catalog);
        translate_book(&catalog, &mut book);
    }

    serde_json::to_writer(io::stdout(), &book)?;

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
