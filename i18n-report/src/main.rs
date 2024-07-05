// Copyright 2024 Google LLC
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

//! Utility to generate an HTML report about the number of translated messages per language from a
//! set of PO files.

mod stats;

use anyhow::{bail, Context as _};
use polib::po_file;
use stats::MessageStats;
use std::{fs, path::Path};
use tera::{Context, Tera};

const REPORT_TEMPLATE: &str = include_str!("../templates/report.html");

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let [_, report_file, translations @ ..] = args.as_slice() else {
        bail!("Usage: {} <report.html> <language.po>...", args[0]);
    };

    let mut languages = translations
        .iter()
        .map(|translation| {
            let catalog = po_file::parse(Path::new(translation))
                .with_context(|| format!("Could not parse {:?}", &translation))?;
            let stats = MessageStats::for_catalog(&catalog);
            Ok::<_, anyhow::Error>(stats)
        })
        .collect::<Result<Vec<_>, _>>()?;
    languages.sort_by_key(|stats| stats.translated_count);
    languages.reverse();
    let languages = languages
        .iter()
        .map(MessageStats::to_context)
        .collect::<Vec<_>>();

    let mut context = Context::new();
    context.insert("languages", &languages);
    let report = Tera::one_off(REPORT_TEMPLATE, &context, true)?;
    fs::write(report_file, report)?;

    Ok(())
}
