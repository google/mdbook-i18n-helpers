#![no_main]

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::gettext::translate_book;
use mdbook_i18n_helpers_fuzz::{create_book, create_catalog, BookItem};

fuzz_target!(|inputs: (Vec<(&str, &str)>, Vec<BookItem>)| {
    let (translations, book_items) = inputs;
    let catalog = create_catalog(translations);
    let mut book = create_book(book_items);
    let _ = translate_book(&catalog, &mut book); // Err(_) can happen and it's fine.
});
