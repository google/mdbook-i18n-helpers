use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tera::Tera;

pub struct CustomComponent {
    template: Tera,
    name: String,
}

impl CustomComponent {
    pub fn new(name: &str, template: Tera) -> Result<CustomComponent> {
        Ok(CustomComponent {
            name: String::from(name),
            template,
        })
    }

    pub fn register_function(&mut self, name: &str, function: impl tera::Function + 'static) {
        self.template.register_function(name, function);
    }

    pub fn render(
        &self,
        tera_context: &tera::Context,
        attributes: BTreeMap<String, String>,
    ) -> Result<String> {
        let mut tera_context = tera_context.clone();
        tera_context.insert("attributes", &attributes);
        let output = self.template.render(&self.name, &tera_context);

        if let Err(err) = &output {
            println!("Error rendering component {}: {:?}", self.name, err);
        }

        Ok(output?)
    }

    pub fn component_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Component {
    Named { name: String, path: PathBuf },
    Anonymous(PathBuf),
}

/// Configuration in `book.toml` `[output.tera-renderer]`.
#[derive(Deserialize)]
pub struct TeraRendererConfig {
    /// Relative path to the templates directory from the `book.toml` directory.
    pub templates_dir: PathBuf,
    /// Custom HTML components to register.
    #[serde(default)]
    pub html_components: Vec<Component>,
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

    fn create_custom_components(
        &self,
        tera_template: &Tera,
        current_dir: &Path,
    ) -> Result<Vec<CustomComponent>> {
        self.html_components
            .iter()
            .map(|component| {
                Ok(match component {
                    Component::Named { name, path } => {
                        let mut template = tera_template.clone();
                        template.add_template_file(path, Some(name))?;
                        CustomComponent::new(name, template)?
                    }
                    Component::Anonymous(path) => {
                        let mut template = tera_template.clone();
                        let name = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap_or_default();
                        template.add_template_file(current_dir.join(path), Some(name))?;
                        CustomComponent::new(name, template)?
                    }
                })
            })
            .collect()
    }

    pub fn create_template_and_components(
        &self,
        current_dir: &Path,
    ) -> Result<(Tera, Vec<CustomComponent>)> {
        let mut tera_template = Tera::default();
        Self::add_templates_recursively(
            &mut tera_template,
            &current_dir.join(&self.templates_dir),
        )?;
        let components = self.create_custom_components(&tera_template, current_dir)?;

        Ok((tera_template, components))
    }
}
