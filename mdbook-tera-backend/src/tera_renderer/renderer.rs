use anyhow::{anyhow, Result};
use mdbook::renderer::RenderContext;
use serde_json::to_value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tera::Tera;

/// Renderer for the tera backend.
///
/// This will read all the files in the `RenderContext` and render them using the `Tera` template.
///
/// # Example
///
/// ```
/// let mut stdin = io::stdin();
/// let ctx = RenderContext::from_json(&mut stdin).unwrap();
/// let config: TeraRendererConfig = ctx
///     .config
///     .get_deserialized_opt("output.tera-backend")
///     .expect("Failed to get tera-backend config")
///     .unwrap();
///
/// let tera_template = config
///    .create_template(&ctx.root)
///    .expect("Failed to create components");
/// let mut renderer = Renderer::new(ctx, tera_template).expect("Failed to create renderer");
/// renderer.render_book().expect("Failed to render book");
/// ```
pub(crate) struct Renderer {
    ctx: Arc<RenderContext>,
    serialized_ctx: serde_json::Value,
    counter: u64,
    tera_template: Tera,
}

impl Renderer {
    /// Create a new `Renderer` from the `RenderContext` and `Tera` template.
    ///
    /// # Arguments
    ///
    /// `ctx`: The `RenderContext` to be used for rendering. This is usually obtained from `stdin`.
    /// `tera_template`: A pre-configured `Tera` template.
    pub(crate) fn new(ctx: RenderContext, tera_template: Tera) -> Result<Renderer> {
        let mut renderer = Renderer {
            serialized_ctx: serde_json::to_value(&ctx)?,
            ctx: Arc::new(ctx),
            counter: 0,
            tera_template,
        };
        renderer
            .tera_template
            .register_function("get_context", renderer.create_get_context_function());
        Ok(renderer)
    }

    /// Render the book.
    pub(crate) fn render_book(&mut self) -> Result<()> {
        let dest_dir = self.ctx.destination.parent().unwrap().to_owned();
        if !dest_dir.is_dir() {
            return Err(anyhow!("{dest_dir:?} is not a directory"));
        }
        self.render_book_directory(&dest_dir)
    }

    /// Create the `get_context` function for the `Tera` template, a helper that allows retrieving values from `ctx`.
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

    /// Render the book directory located at `path` recursively.
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

    /// Reads the file at `path` and renders it.
    fn process_file(&mut self, path: &Path) -> Result<()> {
        if path.extension().unwrap_or_default() != "html" {
            return Ok(());
        }
        let file_content = std::fs::read_to_string(path)?;
        let output = self.render_file_content(&file_content, path)?;
        let mut output_file = fs::File::create(path)?;
        output_file.write_all(output.as_bytes())?;
        Ok(())
    }

    /// Creates the rendering context to be passed to the templates.
    ///
    /// # Arguments
    ///
    /// `path`: The path to the file that will be added as extra context to the renderer.
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

    /// Rendering logic for an individual file.
    ///
    /// # Arguments
    ///
    /// `file_content`: The content of the file to be rendered.
    /// `path`: The path of the file to be rendered.
    ///
    /// # Returns
    ///
    /// The rendered file.
    fn render_file_content(&mut self, file_content: &str, path: &Path) -> Result<String> {
        let tera_context = self.create_context(path);

        let rendered_file = self
            .tera_template
            .render_str(file_content, &tera_context)
            .map_err(|e| anyhow!("Error rendering file {path:?}: {e:?}"))?;
        Ok(rendered_file)
    }
}
