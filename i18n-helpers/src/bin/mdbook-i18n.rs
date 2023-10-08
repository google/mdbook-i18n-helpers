use anyhow::{anyhow, Result};
use mdbook::renderer::RenderContext;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io;

/// Parameters for the i18n renderer.
///
/// They are set in the `output.i18n` section of the book's `book.toml` file.
#[derive(Deserialize)]
struct I18nConfiguration {
    /// A map of language codes to language names.
    ///
    /// ## Example
    ///
    /// ```toml
    /// [output.i18n.languages]
    /// "en" = "English"
    /// "es" = "Spanish (Español)"
    /// "ko" = "Korean (한국어)"
    /// "pt-BR" = "Brazilian Portuguese (Português do Brasil)"
    /// ```
    #[serde(default)]
    languages: BTreeMap<String, String>,
    /// Default language code. It will not be translated.
    default_language: Option<String>,

    /// Whether to translate all languages or just the selected language in `mdbook.config.book.language`, defaults to false.
    #[serde(default)]
    translate_all_languages: bool,
    /// Whether to move the translations to their renderer's directory, defaults to false.
    ///
    /// By default, translations' output will live in `book/i18n/<language>/<renderer>`.
    /// For all renderers in this list, we will move individual translations to `book/<renderer>/<language>`.
    #[serde(default)]
    move_translations_directories: Vec<String>,
}

fn main() -> Result<()> {
    let mut stdin = io::stdin();

    // Get the configs
    let ctx = RenderContext::from_json(&mut stdin)?;
    let i18n_config: I18nConfiguration = ctx
        .config
        .get_deserialized_opt("output.i18n")?
        .ok_or_else(|| anyhow!("No output.i18n config in book.toml"))?;

    if !i18n_config.translate_all_languages {
        return Ok(());
    }

    let mut mdbook = mdbook::MDBook::load(&ctx.root)?;
    // Overwrite with current values from stdin. This is necessary because mdbook will add data to the config.
    mdbook.book = ctx.book.clone();
    mdbook.config = ctx.config.clone();
    mdbook.root = ctx.root.clone();

    let book_config = mdbook
        .config
        .get_mut("output.i18n")
        .ok_or_else(|| anyhow!("No output.i18n config in book.toml"))?;
    // Set translate_all_languages to false for nested builds to prevent infinite recursion.
    book_config
        .as_table_mut()
        .ok_or_else(|| anyhow!("output.i18n config in book.toml is not a table"))?
        .insert(String::from("translate_all_languages"), false.into());

    let output_directory = ctx.destination;
    let default_language = &i18n_config.default_language;

    for language in i18n_config.languages.keys() {
        // Skip current language and default language.
        if Some(language) == ctx.config.book.language.as_ref() {
            continue;
        }
        if default_language.as_ref() == Some(language) {
            continue;
        }
        let translation_path = output_directory.join(language);

        // Book doesn't implement clone, so we just mutate in place.
        mdbook.config.book.language = Some(language.clone());
        mdbook.config.book.multilingual = true;
        mdbook.config.build.build_dir = translation_path;
        mdbook.build()?;
        for renderer in &i18n_config.move_translations_directories {
            std::fs::create_dir_all(
                output_directory
                    .parent()
                    .ok_or_else(|| anyhow!("Failed to retrieve parent directory"))?
                    .join(renderer),
            )?;
            std::fs::rename(
                output_directory.join(language).join(renderer),
                output_directory
                    .parent()
                    .ok_or_else(|| anyhow!("Failed to retrieve parent directory"))?
                    .join(renderer)
                    .join(language),
            )?;
        }
    }
    Ok(())
}
