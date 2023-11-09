use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tera::Tera;

/// Configuration in `book.toml` `[output.tera-renderer]`.
#[derive(Deserialize)]
pub struct TeraRendererConfig {
    /// Relative path to the templates directory from the `book.toml` directory.
    pub template_dir: Option<PathBuf>,
}

/// Recursively add all templates in the `template_dir` to the `tera_template`.
fn add_templates_recursively(tera_template: &mut Tera, directory: &Path) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            add_templates_recursively(tera_template, &path)?;
        } else {
            tera_template.add_template_file(&path, path.file_name().unwrap().to_str())?;
        }
    }
    Ok(())
}

impl TeraRendererConfig {
    /// Create the `tera_template` and add all templates in the `template_dir` to it.
    pub fn create_template(&self, current_dir: &Path) -> Result<Tera> {
        let mut tera_template = Tera::default();
        if let Some(template_dir) = &self.template_dir {
            add_templates_recursively(&mut tera_template, &current_dir.join(template_dir))?;
        }

        Ok(tera_template)
    }
}
