use anyhow::{anyhow, Result};
use mdbook_renderer::RenderContext;
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
    use super::*;
    use crate::tera_renderer::custom_component::TeraRendererConfig;
    use anyhow::Context;
    use mdbook_driver::MDBook;

    fn create_render_context(
        files: &[(&str, &str)],
    ) -> anyhow::Result<(RenderContext, tempfile::TempDir)> {
        let tmpdir = tempfile::tempdir().context("Could not create temporary directory")?;

        for (path, contents) in files {
            let path = tmpdir.path().join(path);
            let dir = path
                .parent()
                .with_context(|| format!("Could not find parent in {}", path.display()))?;
            std::fs::create_dir_all(dir)
                .with_context(|| format!("Could not create {}", dir.display()))?;
            std::fs::write(&path, contents)
                .with_context(|| format!("Could not write {}", path.display()))?;
        }

        let mdbook = MDBook::load(tmpdir.path()).context("Could not load book")?;
        let dest = mdbook.build_dir_for("tera-backend");
        let ctx = RenderContext::new(mdbook.root, mdbook.book, mdbook.config, dest);
        Ok((ctx, tmpdir))
    }

    #[test]
    fn test_renderer() -> anyhow::Result<()> {
        let (ctx, tmpdir) = create_render_context(&[
            (
                "book.toml",
                r#"
                    [book]
                    title = "Foo"

                    [output.html]

                    [output.tera-backend]
                    template_dir = "templates"
                "#,
            ),
            ("src/SUMMARY.md", ""),
            (
                "book/html/test.html",
                r#"
                    {% include "test_template.html" %}
                    Path: {{ path }}
                "#,
            ),
            ("templates/test_template.html", "From test_template"),
        ])?;

        let config: TeraRendererConfig = ctx
            .config
            .get("output.tera-backend")?
            .ok_or_else(|| anyhow!("No tera backend configuration."))?;

        let tera_template = config.create_template(&ctx.root)?;
        let mut renderer = Renderer::new(ctx, tera_template);
        renderer.render_book().expect("Failed to render book");

        assert_eq!(
            std::fs::read_to_string(tmpdir.path().join("book/html/test.html"))?,
            r"
                    From test_template
                    Path: html/test.html
                "
        );
        Ok(())
    }
}
