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

#[cfg(test)]
mod tests {
    use super::*;
    use polib::{message::Message, metadata::CatalogMetadata};

    fn sample_stats() -> MessageStats {
        MessageStats {
            language: "fr".to_string(),
            pot_creation_date: "2026-01-02 03:04+0000".to_string(),
            non_translated_count: 2,
            translated_count: 3,
            fuzzy_non_translated_count: 4,
            fuzzy_translated_count: 7,
        }
    }

    #[test]
    fn empty_catalog() {
        let mut metadata = CatalogMetadata::new();
        metadata.language = "fr".to_string();
        metadata.pot_creation_date = "2026-01-02 03:04+0000".to_string();
        let catalog = Catalog::new(metadata);

        assert_eq!(
            MessageStats::for_catalog(&catalog),
            MessageStats {
                language: "fr".to_string(),
                pot_creation_date: "2026-01-02 03:04+0000".to_string(),
                ..MessageStats::default()
            }
        );
    }

    #[test]
    fn catalog_counts_translation_statuses() {
        let mut catalog = Catalog::new(CatalogMetadata::new());
        for (count, translation, flags) in [
            (1, "", ""),
            (2, "Bonjour", ""),
            (3, "", "fuzzy"),
            (4, "Bonjour", "fuzzy"),
        ] {
            for index in 0..count {
                catalog.append_or_update(
                    Message::build_singular()
                        .with_msgid(format!("Message {count}-{index}"))
                        .with_msgstr(translation.to_string())
                        .with_flags(flags.parse().unwrap())
                        .done(),
                );
            }
        }

        assert_eq!(
            MessageStats::for_catalog(&catalog),
            MessageStats {
                language: catalog.metadata.language.clone(),
                pot_creation_date: catalog.metadata.pot_creation_date.clone(),
                non_translated_count: 1,
                translated_count: 2,
                fuzzy_non_translated_count: 3,
                fuzzy_translated_count: 4,
            }
        );
    }

    #[test]
    fn catalog_counts_plural_messages() {
        let mut catalog = Catalog::new(CatalogMetadata::new());
        for (index, (translations, flags)) in [
            (["", ""], ""),
            (["Un", ""], ""),
            (["Un", "Plusieurs"], ""),
            (["", ""], "fuzzy"),
            (["", "Plusieurs"], "fuzzy"),
            (["Un", "Plusieurs"], "fuzzy"),
        ]
        .into_iter()
        .enumerate()
        {
            catalog.append_or_update(
                Message::build_plural()
                    .with_msgid(format!("One {index}"))
                    .with_msgid_plural(format!("Many {index}"))
                    .with_msgstr_plural(translations.into_iter().map(String::from).collect())
                    .with_flags(flags.parse().unwrap())
                    .done(),
            );
        }

        assert_eq!(
            MessageStats::for_catalog(&catalog),
            MessageStats {
                language: catalog.metadata.language.clone(),
                pot_creation_date: catalog.metadata.pot_creation_date.clone(),
                non_translated_count: 2,
                translated_count: 1,
                fuzzy_non_translated_count: 2,
                fuzzy_translated_count: 1,
            }
        );
    }

    #[test]
    fn total_includes_all_statuses() {
        assert_eq!(MessageStats::default().total(), 0);
        assert_eq!(sample_stats().total(), 16);
    }

    #[test]
    fn template_context() {
        let expected = BTreeMap::from([
            ("language".to_string(), "fr".into()),
            (
                "pot_creation_date".to_string(),
                "2026-01-02 03:04+0000".into(),
            ),
            ("non_translated_count".to_string(), 2u32.into()),
            ("translated_count".to_string(), 3u32.into()),
            ("fuzzy_non_translated_count".to_string(), 4u32.into()),
            ("fuzzy_translated_count".to_string(), 7u32.into()),
            ("non_translated_percent".to_string(), 12.5.into()),
            ("translated_percent".to_string(), 18.75.into()),
            ("fuzzy_non_translated_percent".to_string(), 25.0.into()),
            ("fuzzy_translated_percent".to_string(), 43.75.into()),
            ("total".to_string(), 16u32.into()),
        ]);

        assert_eq!(sample_stats().to_context(), expected);
    }

    #[test]
    fn display_summary() {
        assert_eq!(
            sample_stats().to_string(),
            "fr: 3 (7, 4) / 16, creation date 2026-01-02 03:04+0000"
        );
    }
}
