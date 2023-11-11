use anyhow::{anyhow, Result};
use mdbook::renderer::RenderContext;
use std::path::Path;
use tera::Tera;

/// Renderer for the tera backend.
///
/// This will read all the files in the `RenderContext` and render them using the `Tera` template.
/// ```
pub struct Renderer {
    ctx: RenderContext,
    tera_template: Tera,
}

impl Renderer {
    /// Create a new `Renderer` from the `RenderContext` and `Tera` template.
    pub fn new(ctx: RenderContext, tera_template: Tera) -> Self {
        Renderer { ctx, tera_template }
    }

    /// Render the book. This goes through the output of the HTML renderer
    /// by considering all the output HTML files as input to the Tera template.
    /// It overwrites the preexisting files with their Tera-rendered version.
    pub fn render_book(&mut self) -> Result<()> {
        let dest_dir = self.ctx.destination.parent().unwrap().join("html");
        if !dest_dir.is_dir() {
            return Err(anyhow!(
                "{dest_dir:?} is not a directory. Please make sure the HTML renderer is enabled."
            ));
        }
        self.render_book_directory(&dest_dir)
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
        Ok(std::fs::write(path, output)?)
    }

    /// Creates the rendering context to be passed to the templates.
    ///
    /// # Arguments
    ///
    /// `path`: The path to the file that will be added as extra context to the renderer.
    fn create_context(&mut self, path: &Path) -> tera::Context {
        let mut context = tera::Context::new();
        let book_dir = self.ctx.destination.parent().unwrap();
        let relative_path = path.strip_prefix(book_dir).unwrap();
        context.insert("path", &relative_path);
        context.insert("book_dir", &self.ctx.destination.parent().unwrap());

        context
    }

    /// Rendering logic for an individual file.
    fn render_file_content(&mut self, file_content: &str, path: &Path) -> Result<String> {
        let tera_context = self.create_context(path);

        let rendered_file = self
            .tera_template
            .render_str(file_content, &tera_context)
            .map_err(|e| anyhow!("Error rendering file {path:?}: {e:?}"))?;
        Ok(rendered_file)
    }
}

#[cfg(test)]
mod test {
    use tempdir::TempDir;

    use super::*;
    use crate::tera_renderer::custom_component::TeraRendererConfig;
    use anyhow::Result;

    const RENDER_CONTEXT_STR: &str = r#"
    {
        "version":"0.4.32",
        "root":"",
        "book":{
           "sections": [],
           "__non_exhaustive": null
        },
        "destination": "",
        "config":{
           "book":{
              "authors":[
                 "Martin Geisler"
              ],
              "language":"en",
              "multilingual":false,
              "src":"src",
              "title":"Comprehensive Rust 🦀"
           },
           "build":{
              "build-dir":"book",
              "use-default-preprocessors":true
           },
           "output":{
              "tera-backend": {
                    "template_dir": "templates"
              },
              "renderers":[
                 "html",
                 "tera-backend"
              ]
           }
        }
     }"#;

    const HTML_FILE: &str = r#"
        <!DOCTYPE html>
            {% include "test_template.html" %}
            PATH: {{ path }}
        </html>
    "#;

    const TEMPLATE_FILE: &str = "RENDERED";

    const RENDERED_HTML_FILE: &str = r"
        <!DOCTYPE html>
            RENDERED
            PATH: html/test.html
        </html>
    ";

    #[test]
    fn test_renderer() -> Result<()> {
        let mut ctx = RenderContext::from_json(RENDER_CONTEXT_STR.as_bytes()).unwrap();

        let tmp_dir = TempDir::new("output")?;
        let html_path = tmp_dir.path().join("html");
        let templates_path = tmp_dir.path().join("templates");

        std::fs::create_dir(&html_path)?;
        std::fs::create_dir(&templates_path)?;

        let html_file_path = html_path.join("test.html");
        std::fs::write(&html_file_path, HTML_FILE)?;
        std::fs::write(templates_path.join("test_template.html"), TEMPLATE_FILE)?;

        ctx.destination = tmp_dir.path().join("tera-renderer");
        ctx.root = tmp_dir.path().to_owned();

        let config: TeraRendererConfig = ctx
            .config
            .get_deserialized_opt("output.tera-backend")?
            .ok_or_else(|| anyhow!("No tera backend configuration."))?;

        let tera_template = config.create_template(&ctx.root)?;
        let mut renderer = Renderer::new(ctx, tera_template);
        renderer.render_book().expect("Failed to render book");

        assert_eq!(std::fs::read_to_string(html_file_path)?, RENDERED_HTML_FILE);
        Ok(())
    }
}
