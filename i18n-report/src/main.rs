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
    fs::{self, read_dir},
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
        Args::Diff {
            old_translations,
            new_translations,
        } => {
            diff(&old_translations, &new_translations)?;
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
    /// Generate a report showing any difference between two directories of language files.
    Diff {
        /// Directory containing the old translation .po files.
        old_translations: PathBuf,
        /// Directory containing the new translation .po files.
        new_translations: PathBuf,
    },
}

/// Generates an HTML report about the status of translations in each of the given language files.
fn report(report_file: &Path, translation_files: &[PathBuf]) -> anyhow::Result<()> {
    let mut languages = all_stats(translation_files)?;
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

/// Reads each given PO file and returns message stats for each.
fn all_stats(files: &[PathBuf]) -> anyhow::Result<Vec<MessageStats>> {
    files
        .iter()
        .map(|translation| {
            let catalog = po_file::parse(translation)
                .with_context(|| format!("Could not parse {:?}", &translation))?;
            let stats = MessageStats::for_catalog(&catalog);
            Ok(stats)
        })
        .collect()
}

/// Prints a report showing any difference between two directories of language files.
#[allow(clippy::print_stdout)]
fn diff(
    old_translations_directory: &Path,
    new_translations_directory: &Path,
) -> anyhow::Result<()> {
    let mut old_translations = all_stats(&po_files(old_translations_directory)?)?;
    let mut new_translations = all_stats(&po_files(new_translations_directory)?)?;
    old_translations.sort_by_key(|stats| stats.language.clone());
    new_translations.sort_by_key(|stats| stats.language.clone());

    if old_translations == new_translations {
        return Ok(());
    }

    let mut old_iter = old_translations.iter();
    let mut new_iter = new_translations.iter();
    let mut old = old_iter.next();
    let mut new = new_iter.next();
    println!("Counts are \"translated (fuzzy, fuzzy untranslated) / total\"");
    loop {
        match (old, new) {
            (None, None) => break,
            (Some(old_stats), None) => {
                println!("Removed {}", old_stats);
                old = old_iter.next();
            }
            (None, Some(new_stats)) => {
                println!("Added {}", new_stats);
                new = new_iter.next();
            }
            (Some(old_stats), Some(new_stats)) => match old_stats.language.cmp(&new_stats.language)
            {
                std::cmp::Ordering::Less => {
                    println!("Removed {}", old_stats);
                    old = old_iter.next();
                }
                std::cmp::Ordering::Greater => {
                    println!("Added {}", new_stats);
                    new = new_iter.next();
                }
                std::cmp::Ordering::Equal => {
                    if old_stats != new_stats {
                        println!("Changed {} -> {}", old_stats, new_stats);
                    }
                    old = old_iter.next();
                    new = new_iter.next();
                }
            },
        }
    }

    Ok(())
}

/// Given a directory path, returns the paths of all the `.po` files in it.
fn po_files(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    read_dir(directory)?
        .filter_map(|entry| {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Some(Err(e.into())),
            };
            let path = entry.path();
            if path.extension()? == "po" {
                Some(Ok(path))
            } else {
                None
            }
        })
        .collect()
}
