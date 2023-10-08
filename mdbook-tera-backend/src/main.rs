mod tera_renderer;

use mdbook::renderer::RenderContext;
use std::io;

use crate::tera_renderer::*;

fn main() {
    let mut stdin = io::stdin();
    // Get the configs
    let ctx = RenderContext::from_json(&mut stdin).unwrap();
    let config: TeraRendererConfig = ctx
        .config
        .get_deserialized_opt("output.tera-backend")
        .expect("Failed to get Gaia config")
        .unwrap();

    let (tera_template, components) = config
        .create_template_and_components(&ctx.root)
        .expect("Failed to create components");

    let mut renderer = Renderer::new(ctx, tera_template).expect("Failed to create renderer");

    for component in components {
        renderer.add_component(component);
    }

    renderer.render_book().expect("Failed to render book");
}
