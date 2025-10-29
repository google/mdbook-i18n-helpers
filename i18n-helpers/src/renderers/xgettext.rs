use crate::xgettext::create_catalogs;
use anyhow::Context;
use mdbook_renderer::{RenderContext, Renderer};
use std::fs;

/// Renderer for xgettext
pub struct Xgettext;

impl Renderer for Xgettext {
    fn name(&self) -> &str {
        "xgettext"
    }

    fn render(&self, ctx: &RenderContext) -> anyhow::Result<()> {
        fs::create_dir_all(&ctx.destination)
            .with_context(|| format!("Could not create {}", ctx.destination.display()))?;
        let catalogs =
            create_catalogs(ctx, std::fs::read_to_string).context("Extracting messages")?;

        // Create a template file for each entry with the content from the respective catalog.
        for (file_path, catalog) in catalogs {
            let dst_path = ctx.destination.join(file_path);
            let directory_path = dst_path.parent().unwrap();
            fs::create_dir_all(directory_path)
                .with_context(|| format!("Could not create {}", directory_path.display()))?;

            polib::po_file::write(&catalog, &dst_path)
                .with_context(|| format!("Writing messages to {}", dst_path.display()))?;
        }

        Ok(())
    }
}
