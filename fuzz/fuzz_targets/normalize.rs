#![no_main]

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::normalize::normalize;
use mdbook_i18n_helpers_fuzz::create_catalog_with_sources;

fuzz_target!(|entries: Vec<(&str, &str, &str)>| {
    let catalog = create_catalog_with_sources(entries);
    let _ = normalize(catalog);
});
