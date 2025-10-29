#![no_main]

use std::path::PathBuf;
use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::xgettext::create_catalogs;
use mdbook_i18n_helpers_fuzz::{create_book, BookItem};
use mdbook_renderer::config::Config;
use mdbook_renderer::RenderContext;

fuzz_target!(|inputs: (&str, Vec<BookItem>)| {
    let (summary, book_items) = inputs;

    let book = create_book(book_items);

    let ctx = RenderContext::new(PathBuf::new(), book, Config::from_str("").unwrap(), "");

    let _ = create_catalogs(&ctx, |_| Ok(summary.to_string()));
});
