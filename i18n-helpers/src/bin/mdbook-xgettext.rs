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
use mdbook_i18n_helpers::xgettext::create_catalog;
use std::{fs, io};

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
    let catalog = create_catalog(&ctx, std::fs::read_to_string).context("Extracting messages")?;
    polib::po_file::write(&catalog, &output_path)
        .with_context(|| format!("Writing messages to {}", output_path.display()))?;

    Ok(())
}
