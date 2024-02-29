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
//! `po/messages.pot`). When the `depth` parameter is included, a new
//! directory will contain the template files split based on the tiers
//! of Chapter nesting.

use anyhow::Context;
use mdbook::renderer::RenderContext;
use mdbook_i18n_helpers::xgettext::create_catalogs;
use std::{fs, io};

fn main() -> anyhow::Result<()> {
    let ctx = RenderContext::from_json(&mut io::stdin()).context("Parsing stdin")?;
    fs::create_dir_all(&ctx.destination)
        .with_context(|| format!("Could not create {}", ctx.destination.display()))?;
    let catalogs = create_catalogs(&ctx, std::fs::read_to_string).context("Extracting messages")?;

    // Create a template file for each entry with the content from the respective catalog.
    for (file_path, catalog) in catalogs {
        let directory_path = file_path.parent().unwrap();
        fs::create_dir_all(directory_path)
            .with_context(|| format!("Could not create {}", directory_path.display()))?;

        polib::po_file::write(&catalog, &file_path)
            .with_context(|| format!("Writing messages to {}", file_path.display()))?;
    }

    Ok(())
}
