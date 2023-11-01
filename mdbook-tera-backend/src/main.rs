mod tera_renderer;

use anyhow::anyhow;
use mdbook::renderer::RenderContext;
use std::io;

use crate::tera_renderer::custom_component::TeraRendererConfig;
use crate::tera_renderer::renderer::Renderer;

/// Re-renders HTML files outputed by the HTML backend with Tera templates.
/// Please make sure the HTML backend is enabled.
fn main() -> anyhow::Result<()> {
    let mut stdin = io::stdin();
    let ctx = RenderContext::from_json(&mut stdin).unwrap();
    if ctx.config.get_preprocessor("html").is_none() {
        return Err(anyhow!(
            "Could not find the HTML backend. Please make sure the HTML backend is enabled."
        ));
    }
    let config: TeraRendererConfig = ctx
        .config
        .get_deserialized_opt("output.tera-backend")
        .expect("Failed to get tera-backend config")
        .unwrap();

    let tera_template = config
        .create_template(&ctx.root)
        .expect("Failed to create components");

    let mut renderer = Renderer::new(ctx, tera_template).expect("Failed to create renderer");

    renderer.render_book().expect("Failed to render book");

    Ok(())
}
