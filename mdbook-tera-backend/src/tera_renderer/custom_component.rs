use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tera::Tera;

/// Configuration in `book.toml` `[output.tera-renderer]`.
#[derive(Deserialize)]
pub struct TeraRendererConfig {
    /// Relative path to the templates directory from the `book.toml` directory.
    pub templates_dir: PathBuf,
}

impl TeraRendererConfig {
    /// Recursively add all templates in the `templates_dir` to the `tera_template`.
    fn add_templates_recursively(tera_template: &mut Tera, directory: &Path) -> Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::add_templates_recursively(tera_template, &path)?;
            } else {
                tera_template.add_template_file(&path, path.file_name().unwrap().to_str())?;
            }
        }
        Ok(())
    }

    /// Create the `tera_template` and add all templates in the `templates_dir` to it.
    pub fn create_template(&self, current_dir: &Path) -> Result<Tera> {
        let mut tera_template = Tera::default();
        Self::add_templates_recursively(
            &mut tera_template,
            &current_dir.join(&self.templates_dir),
        )?;

        Ok(tera_template)
    }
}
