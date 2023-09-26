use serde::Deserialize;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use tera::Tera;

use crate::{RendererError, Result};

use super::RenderingContext;

fn make_strip_prefix_function() -> impl tera::Function {
    move |args: &HashMap<String, serde_json::value::Value>| -> tera::Result<tera::Value> {
        let string = args
            .get("s")
            .ok_or_else(|| tera::Error::from(format!("No s argument provided")))?
            .as_str()
            .ok_or_else(|| tera::Error::from(format!("S has invalid type, expected string")))?;
        let prefix = args
            .get("prefix")
            .ok_or_else(|| tera::Error::from(format!("No prefix argument provided")))?
            .as_str()
            .ok_or_else(|| {
                tera::Error::from(format!("Prefix has invalid type, expected string"))
            })?;
        string
            .strip_prefix(prefix)
            .map(|s| tera::Value::String(s.to_owned()))
            .ok_or_else(|| tera::Error::from(format!("Could not strip prefix")))
    }
}

pub struct CustomComponent {
    template: Tera,
    name: String,
    /// Used to generate unique ids for each component to prevent collisions in javascript with query selectors.
    counter: RefCell<u32>,
}

impl CustomComponent {
    pub fn new(name: &str, template_str: &str, dependencies: &[&Self]) -> Result<CustomComponent> {
        let mut template = Tera::default();
        for dep in dependencies {
            template.extend(&dep.template)?;
        }
        template.add_raw_template(name, template_str)?;
        template.register_function("strip_prefix", make_strip_prefix_function());

        Ok(CustomComponent {
            name: String::from(name),
            counter: RefCell::new(0),
            template,
        })
    }

    pub fn register_function(&mut self, name: &str, function: impl tera::Function + 'static) {
        self.template.register_function(name, function);
    }

    fn create_context(
        &self,
        rendering_context: &RenderingContext,
        attributes: BTreeMap<String, String>,
    ) -> tera::Context {
        let counter = self.counter.replace_with(|&mut counter| counter + 1);
        let mut context = tera::Context::new();
        context.insert("counter", &counter);
        context.insert("language", &rendering_context.language);
        context.insert("path", &rendering_context.path);
        context.insert("ctx", &rendering_context.serialized_ctx);
        context.insert(
            "book_dir",
            &rendering_context.ctx.destination.parent().unwrap(),
        );
        context.insert("attributes", &attributes);

        context
    }

    pub fn render(
        &self,
        rendering_context: &RenderingContext,
        attributes: BTreeMap<String, String>,
    ) -> Result<String> {
        let context = self.create_context(rendering_context, attributes);
        let output = self.template.render(&self.name, &context)?;
        Ok(output)
    }

    pub fn component_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Deserialize)]
pub struct TeraComponentConfig {
    pub name: String,
    pub path: PathBuf,

    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Deserialize)]
pub struct TeraRendererConfig {
    pub components: Vec<TeraComponentConfig>,
}

impl TeraRendererConfig {
    pub fn create_components(&self, current_dir: &Path) -> Result<Vec<CustomComponent>> {
        let mut name_to_component = HashMap::new();
        for component in &self.components {
            let component_path = current_dir.join(&component.path);
            let template_str = std::fs::read_to_string(&component_path)?;
            let dependencies = component
                .dependencies
                .iter()
                .map(|name| {
                    name_to_component.get(name).ok_or_else(|| {
                        RendererError::DependencyNotFound(format!(
                            "Could not find depdendency {}",
                            name
                        ))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let new_component =
                CustomComponent::new(&component.name, &template_str, &dependencies)?;
            name_to_component.insert(component.name.clone(), new_component);
        }
        Ok(name_to_component.into_values().collect())
    }
}
