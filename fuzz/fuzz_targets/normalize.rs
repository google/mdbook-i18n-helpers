#![no_main]

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::normalize::normalize;
use mdbook_i18n_helpers_fuzz::create_catalog;

fuzz_target!(|translations: Vec<(&str, &str)>| {
    let catalog = create_catalog(translations);
    let _ = normalize(catalog);
});
