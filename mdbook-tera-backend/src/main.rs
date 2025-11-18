mod tera_renderer;

use anyhow::{anyhow, Context};
use mdbook_renderer::RenderContext;
use std::io;

use crate::tera_renderer::custom_component::TeraRendererConfig;
use crate::tera_renderer::renderer::Renderer;

/// Re-renders HTML files outputed by the HTML backend with Tera templates.
/// Please make sure the HTML backend is enabled.
fn main() -> anyhow::Result<()> {
    let mut stdin = io::stdin();
    let ctx = RenderContext::from_json(&mut stdin).unwrap();
    if !ctx.config.contains_key("output.html") {
        return Err(anyhow!(
            "Could not find the HTML backend. Please make sure the HTML backend is enabled."
        ));
    }
    let config: TeraRendererConfig = ctx
        .config
        .get("output.tera-backend")
        .context("Failed to get tera-backend config")?
        .context("No tera-backend config found")?;

    let tera_template = config
        .create_template(&ctx.root)
        .context("Failed to create components")?;

    let mut renderer = Renderer::new(ctx, tera_template);

    renderer.render_book().context("Failed to render book")?;

    Ok(())
}
