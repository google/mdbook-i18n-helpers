use super::error::RendererError;
use super::CustomComponent;
use crate::tera_renderer::error::Result;
use lol_html::html_content::ContentType;
use lol_html::{element, RewriteStrSettings};
use mdbook::renderer::RenderContext;
use serde_json::to_value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct RenderingContext<'a> {
    pub path: PathBuf,
    pub language: Option<String>,
    pub serialized_ctx: &'a serde_json::Value,
    pub ctx: &'a RenderContext,
}

impl<'a> RenderingContext<'a> {
    fn new(
        path: PathBuf,
        language: Option<String>,
        serialized_ctx: &'a serde_json::Value,
        ctx: &'a RenderContext,
    ) -> Result<Self> {
        Ok(RenderingContext {
            path,
            language,
            serialized_ctx,
            ctx,
        })
    }
}

pub(crate) struct Renderer {
    ctx: Arc<RenderContext>,
    serialized_ctx: serde_json::Value,
    components: Vec<CustomComponent>,
}

impl Renderer {
    pub(crate) fn new(ctx: RenderContext) -> Result<Renderer> {
        Ok(Renderer {
            serialized_ctx: serde_json::to_value(&ctx)?,
            ctx: Arc::new(ctx),
            components: Vec::new(),
        })
    }

    pub(crate) fn add_component(&mut self, mut component: CustomComponent) {
        component.register_function("get_context", self.create_get_context_function());
        self.components.push(component);
    }

    fn create_get_context_function(&self) -> impl tera::Function {
        let ctx_rx = Arc::clone(&self.ctx);
        move |args: &HashMap<String, serde_json::value::Value>| -> tera::Result<tera::Value> {
            let key = args
                .get("key")
                .ok_or_else(|| tera::Error::from(format!("No key argument provided")))?
                .as_str()
                .ok_or_else(|| {
                    tera::Error::from(format!("Key has invalid type, expected string"))
                })?;
            let value = ctx_rx
                .config
                .get(key)
                .ok_or_else(|| tera::Error::from(format!("Could not find key {key} in config")))?;
            let value = to_value(value)?;
            Ok(value)
        }
    }

    pub(crate) fn render_book(&mut self) -> Result<()> {
        let dest_dir = &self
            .ctx
            .destination
            .parent()
            .ok_or_else(|| {
                RendererError::InvalidPath(format!(
                    "Destination directory {:?} has no parent",
                    self.ctx.destination
                ))
            })?
            .to_owned();
        if !dest_dir.is_dir() {
            return Err(RendererError::InvalidPath(format!(
                "{:?} is not a directory",
                dest_dir
            )));
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
        let mut file_content = String::new();
        {
            let mut file = fs::File::open(path)?;
            file.read_to_string(&mut file_content)?;
        }

        let output = self.render_components(&file_content, path)?;
        let mut output_file = fs::File::create(path)?;
        output_file.write_all(output.as_bytes())?;
        Ok(())
    }

    fn render_components(&mut self, file_content: &str, path: &Path) -> Result<String> {
        let rendering_context = RenderingContext::new(
            path.to_owned(),
            self.ctx.config.book.language.clone(),
            &self.serialized_ctx,
            &self.ctx,
        )?;
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
                    let rendered = component.render(&rendering_context, attributes)?;
                    el.replace(&rendered, ContentType::Html);
                    Ok(())
                })
            })
            .collect();
        let output = lol_html::rewrite_str(
            file_content,
            RewriteStrSettings {
                element_content_handlers: custom_components_handlers,
                ..RewriteStrSettings::default()
            },
        )?;
        Ok(output)
    }
}
