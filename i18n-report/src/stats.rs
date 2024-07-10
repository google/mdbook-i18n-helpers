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

use polib::catalog::Catalog;
use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
};
use tera::Value;

/// Counts of translation message statuses.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageStats {
    pub language: String,
    pub pot_creation_date: String,
    pub non_translated_count: u32,
    pub translated_count: u32,
    pub fuzzy_non_translated_count: u32,
    pub fuzzy_translated_count: u32,
}

impl MessageStats {
    /// Returns the total number of messages.
    pub fn total(&self) -> u32 {
        self.non_translated_count
            + self.translated_count
            + self.fuzzy_non_translated_count
            + self.fuzzy_translated_count
    }

    /// Converts the stats to a map of numbers to be used in context for a Tera template.
    pub fn to_context(&self) -> BTreeMap<String, Value> {
        let mut context: BTreeMap<String, Value> = BTreeMap::new();
        context.insert("language".to_string(), self.language.as_str().into());
        context.insert(
            "pot_creation_date".to_string(),
            self.pot_creation_date.as_str().into(),
        );
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

    /// Returns counts of messages statuses in the given catalog.
    pub fn for_catalog(catalog: &Catalog) -> Self {
        let mut stats = Self {
            language: catalog.metadata.language.clone(),
            pot_creation_date: catalog.metadata.pot_creation_date.clone(),
            ..Self::default()
        };
        for message in catalog.messages() {
            if message.is_translated() {
                if message.is_fuzzy() {
                    stats.fuzzy_translated_count += 1;
                } else {
                    stats.translated_count += 1;
                }
            } else if message.is_fuzzy() {
                stats.fuzzy_non_translated_count += 1;
            } else {
                stats.non_translated_count += 1;
            }
        }
        stats
    }
}

impl Display for MessageStats {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}: {} ({}, {}) / {}, creation date {}",
            self.language,
            self.translated_count,
            self.fuzzy_translated_count,
            self.fuzzy_non_translated_count,
            self.total(),
            self.pot_creation_date,
        )
    }
}
