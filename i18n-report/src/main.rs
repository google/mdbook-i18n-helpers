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

use anyhow::Context as _;
use clap::Parser;
use polib::po_file;
use stats::MessageStats;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tera::{Context, Tera};

const REPORT_TEMPLATE: &str = include_str!("../templates/report.html");

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args {
        Args::Report {
            report_file,
            translation_files,
        } => {
            report(&report_file, &translation_files)?;
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Parser)]
enum Args {
    /// Generate an HTML report about the status of translations in each of the given language files.
    Report {
        /// The filename to which to write the report.
        #[arg(id = "report.html")]
        report_file: PathBuf,
        #[arg(id = "language.po")]
        translation_files: Vec<PathBuf>,
    },
}

/// Generates an HTML report about the status of translations in each of the given language files.
fn report(report_file: &Path, translation_files: &[PathBuf]) -> anyhow::Result<()> {
    let mut languages = translation_files
        .iter()
        .map(|translation| {
            let catalog = po_file::parse(translation)
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
