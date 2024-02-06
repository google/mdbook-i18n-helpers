#![no_main]

use std::path::PathBuf;
use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use mdbook::renderer::RenderContext;
use mdbook::Config;
use mdbook_i18n_helpers::xgettext::create_catalog;
use mdbook_i18n_helpers_fuzz::{create_book, BookItem};

fuzz_target!(|inputs: (&str, Vec<BookItem>)| {
    let (summary, book_items) = inputs;

    let book = create_book(book_items);

    let ctx = RenderContext::new(PathBuf::new(), book, Config::from_str("").unwrap(), "");

    let _ = create_catalog(&ctx, |_| Ok(summary.to_string()));
});
