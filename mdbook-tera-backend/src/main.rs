mod tera_renderer;

use mdbook::renderer::RenderContext;
use std::io;

use crate::tera_renderer::custom_component::TeraRendererConfig;
use crate::tera_renderer::renderer::Renderer;

fn main() {
    let mut stdin = io::stdin();
    let ctx = RenderContext::from_json(&mut stdin).unwrap();
    let config: TeraRendererConfig = ctx
        .config
        .get_deserialized_opt("output.tera-backend")
        .expect("Failed to get tera-backend config")
        .unwrap();

    let tera_template = config
        .create_template_and_components(&ctx.root)
        .expect("Failed to create components");

    let mut renderer = Renderer::new(ctx, tera_template).expect("Failed to create renderer");

    renderer.render_book().expect("Failed to render book");
}
