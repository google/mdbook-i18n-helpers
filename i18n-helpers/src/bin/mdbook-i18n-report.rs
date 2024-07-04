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

use anyhow::{bail, Context as _};
use polib::{catalog::Catalog, po_file};
use std::{collections::BTreeMap, fs, path::Path};
use tera::{Context, Tera, Value};

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let [_, report_file, translations @ ..] = args.as_slice() else {
        bail!("Usage: {} <report.html> <language.po>...", args[0]);
    };

    let mut languages = translations
        .into_iter()
        .map(|translation| {
            let catalog = po_file::parse(Path::new(translation))
                .with_context(|| format!("Could not parse {:?}", &translation))?;
            println!("Read {} messages from {}", catalog.count(), translation);
            let stats = counts(&catalog);
            Ok::<_, anyhow::Error>((catalog.metadata.language, stats))
        })
        .collect::<Result<Vec<_>, _>>()?;
    languages.sort_by_key(|(_, stats)| stats.translated_count);
    languages.reverse();
    let languages = languages
        .into_iter()
        .map(|(language, stats)| (language, stats.to_context()))
        .collect::<Vec<_>>();

    let tera = Tera::new("templates/*.html")?;
    let mut context = Context::new();
    context.insert("languages", &languages);
    let report = tera.render("report.html", &context)?;
    fs::write(report_file, report)?;

    Ok(())
}

/// Counts of translation message statuses.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MessageStats {
    pub non_translated_count: u32,
    pub translated_count: u32,
    pub fuzzy_non_translated_count: u32,
    pub fuzzy_translated_count: u32,
}

impl MessageStats {
    /// Returns the total number of messages.
    fn total(&self) -> u32 {
        self.non_translated_count
            + self.translated_count
            + self.fuzzy_non_translated_count
            + self.fuzzy_translated_count
    }

    /// Converts the stats to a map of numbers to be used in context for a Tera template.
    fn to_context(&self) -> BTreeMap<String, Value> {
        let mut context: BTreeMap<String, Value> = BTreeMap::new();
        context.insert(
            "non_translated_count".to_string(),
            self.non_translated_count.into(),
        );
        context.insert("translated_count".to_string(), self.translated_count.into());
        context.insert(
            "fuzzy_non_translated_count".to_string(),
            self.fuzzy_non_translated_count.into(),
        );
        context.insert(
            "fuzzy_translated_count".to_string(),
            self.fuzzy_translated_count.into(),
        );
        context.insert(
            "non_translated_percent".to_string(),
            (100.0 * f64::from(self.non_translated_count) / f64::from(self.total())).into(),
        );
        context.insert(
            "translated_percent".to_string(),
            (100.0 * f64::from(self.translated_count) / f64::from(self.total())).into(),
        );
        context.insert(
            "fuzzy_non_translated_percent".to_string(),
            (100.0 * f64::from(self.fuzzy_non_translated_count) / f64::from(self.total())).into(),
        );
        context.insert(
            "fuzzy_translated_percent".to_string(),
            (100.0 * f64::from(self.fuzzy_translated_count) / f64::from(self.total())).into(),
        );
        context.insert("total".to_string(), self.total().into());
        context
    }
}

/// Returns counts of messages statuses in the given catalog.
fn counts(catalog: &Catalog) -> MessageStats {
    let mut stats = MessageStats::default();
    for message in catalog.messages() {
        if message.is_translated() {
            if message.is_fuzzy() {
                stats.fuzzy_translated_count += 1;
            } else {
                stats.translated_count += 1;
            }
        } else {
            if message.is_fuzzy() {
                stats.fuzzy_non_translated_count += 1;
            } else {
                stats.non_translated_count += 1;
            }
        }
    }
    stats
}
