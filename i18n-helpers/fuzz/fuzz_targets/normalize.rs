#![no_main]

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::normalize::normalize;
use polib::catalog::Catalog;
use polib::message::Message;
use polib::metadata::CatalogMetadata;

fuzz_target!(|translations: Vec<(&str, &str)>| {
    let catalog = create_catalog(translations);
    let _ = normalize(catalog);
});

fn create_catalog(translations: Vec<(&str, &str)>) -> Catalog {
    let mut catalog = Catalog::new(CatalogMetadata::new());
    for (idx, (msgid, msgstr)) in translations.iter().enumerate() {
        let message = Message::build_singular()
            .with_source(format!("foo.md:{idx}"))
            .with_msgid(String::from(*msgid))
            .with_msgstr(String::from(*msgstr))
            .done();
        catalog.append_or_update(message);
    }
    catalog
}
