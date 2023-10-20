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

    pub fn create_template_and_components(&self, current_dir: &Path) -> Result<Tera> {
        let mut tera_template = Tera::default();
        Self::add_templates_recursively(
            &mut tera_template,
            &current_dir.join(&self.templates_dir),
        )?;

        Ok(tera_template)
    }
}
