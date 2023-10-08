use super::custom_component::CustomComponent;
use anyhow::{anyhow, Result};
use lol_html::html_content::ContentType;
use lol_html::{element, RewriteStrSettings};
use mdbook::renderer::RenderContext;
use serde_json::to_value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tera::Tera;

pub(crate) struct Renderer {
    ctx: Arc<RenderContext>,
    serialized_ctx: serde_json::Value,
    components: Vec<CustomComponent>,
    counter: u64,
    tera_template: Tera,
}

impl Renderer {
    pub(crate) fn new(ctx: RenderContext, tera_template: Tera) -> Result<Renderer> {
        let mut renderer = Renderer {
            serialized_ctx: serde_json::to_value(&ctx)?,
            ctx: Arc::new(ctx),
            components: Vec::new(),
            counter: 0,
            tera_template,
        };
        renderer
            .tera_template
            .register_function("get_context", renderer.create_get_context_function());
        Ok(renderer)
    }

    pub(crate) fn add_component(&mut self, mut component: CustomComponent) {
        component.register_function("get_context", self.create_get_context_function());
        self.components.push(component);
    }

    fn create_get_context_function(&self) -> impl tera::Function {
        let ctx_rc = Arc::clone(&self.ctx);
        move |args: &HashMap<String, serde_json::value::Value>| -> tera::Result<tera::Value> {
            let key = args
                .get("key")
                .ok_or_else(|| tera::Error::from(format!("No key argument provided")))?
                .as_str()
                .ok_or_else(|| {
                    tera::Error::from(format!("Key has invalid type, expected string"))
                })?;
            let value = ctx_rc
                .config
                .get(key)
                .ok_or_else(|| tera::Error::from(format!("Could not find key {key} in config")))?;
            let value = to_value(value)?;
            Ok(value)
        }
    }

    pub(crate) fn render_book(&mut self) -> Result<()> {
        let dest_dir = self.ctx.destination.parent().unwrap().to_owned();
        if !dest_dir.is_dir() {
            return Err(anyhow!("{dest_dir:?} is not a directory"));
        }
        self.render_book_directory(&dest_dir)
    }

    fn render_book_directory(&mut self, path: &Path) -> Result<()> {
        for entry in path.read_dir()? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.render_book_directory(&path)?;
            } else {
                self.process_file(&path)?;
            }
        }
        Ok(())
    }

    fn process_file(&mut self, path: &Path) -> Result<()> {
        if path.extension().unwrap_or_default() != "html" {
            return Ok(());
        }
        let file_content = std::fs::read_to_string(path)?;
        let output = self.render_components(&file_content, path)?;
        let mut output_file = fs::File::create(path)?;
        output_file.write_all(output.as_bytes())?;
        Ok(())
    }

    fn create_context(&mut self, path: &Path) -> tera::Context {
        let mut context = tera::Context::new();
        context.insert("path", path);
        context.insert("ctx", &self.serialized_ctx);
        context.insert("book_dir", &self.ctx.destination.parent().unwrap());
        context.insert("counter", &self.counter);
        context.insert("attributes", &BTreeMap::<String, String>::new());
        self.counter += 1;

        context
    }

    fn render_components(&mut self, file_content: &str, path: &Path) -> Result<String> {
        let tera_context = self.create_context(path);

        let rendered_file = self
            .tera_template
            .render_str(file_content, &tera_context)
            .map_err(|e| anyhow!("Error rendering file {path:?}: {e:?}"))?;
        let custom_components_handlers = self
            .components
            .iter()
            .map(|component| {
                element!(component.component_name(), |el| {
                    let attributes: BTreeMap<String, String> = el
                        .attributes()
                        .iter()
                        .map(|attribute| (attribute.name(), attribute.value()))
                        .collect();
                    let rendered = component.render(&tera_context, attributes)?;
                    el.replace(&rendered, ContentType::Html);
                    Ok(())
                })
            })
            .collect();
        let output = lol_html::rewrite_str(
            &rendered_file,
            RewriteStrSettings {
                element_content_handlers: custom_components_handlers,
                ..RewriteStrSettings::default()
            },
        )?;
        Ok(output)
    }
}
