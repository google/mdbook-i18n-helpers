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

use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook_i18n_helpers::preprocessors::Gettext;
use semver::{Version, VersionReq};
use std::{io, process};

/// Execute main logic by this mdbook preprocessor.
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

    let gettext = Gettext;
    let book = gettext.run(&ctx, book)?;

    serde_json::to_writer(io::stdout(), &book)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    if std::env::args().len() == 3 {
        assert_eq!(std::env::args().nth(1).as_deref(), Some("supports"));
        if let Some(renderer) = std::env::args().nth(2).as_deref() {
            let gettext = Gettext;
            if gettext.supports_renderer(renderer) {
                process::exit(0)
            } else {
                process::exit(1)
            }
        } else {
            process::exit(0);
        }
    }

    preprocess()
}
