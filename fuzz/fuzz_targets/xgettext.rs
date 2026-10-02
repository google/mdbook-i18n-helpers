#![no_main]

use std::io::BufWriter;
use std::path::PathBuf;
use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::xgettext::create_catalogs;
use mdbook_i18n_helpers_fuzz::{create_book, BookItem};
use mdbook_renderer::config::Config;
use mdbook_renderer::RenderContext;
use polib::po_file;

fuzz_target!(|inputs: (&str, Vec<BookItem>)| {
    let (summary, book_items) = inputs;

    let book = create_book(book_items);

    let ctx = RenderContext::new(PathBuf::new(), book, Config::from_str("").unwrap(), "");

    if let Ok(catalogs) = create_catalogs(&ctx, |_| Ok(summary.to_string())) {
        for catalog in catalogs.values() {
            let mut output = Vec::new();
            {
                let mut writer = BufWriter::new(&mut output);
                po_file::write(catalog, &mut writer).unwrap();
            }
            po_file::parse_from_reader(output.as_slice()).unwrap();
        }
    }
});
