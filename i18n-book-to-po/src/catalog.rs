use std::path::Path;

use polib::catalog::Catalog;
use polib::message::Message;
use polib::metadata::CatalogMetadata;
use polib::po_file;

// Create a catalog from the translation pairs given.
pub fn update_catalog(
    source_file: &Path,
    mut catalog: Catalog,
    translations: Vec<(usize, (String, String))>,
) -> Catalog {
    for (idx, (msgid, msgstr)) in translations.into_iter() {
        let message = Message::build_singular()
            .with_source(format!(
                "{}:{idx}",
                source_file.file_name().unwrap().to_str().unwrap()
            ))
            .with_msgid(msgid)
            .with_msgstr(msgstr)
            .done();
        catalog.append_or_update(message);
    }
    catalog
}

/// Write the catalog to the provided path
pub fn update_po_file(
    output: &Path,
    source_file: &Path,
    translations: Vec<(usize, (String, String))>,
) -> anyhow::Result<()> {
    let catalog = if output.exists() {
        po_file::parse(output)?
    } else {
        Catalog::new(CatalogMetadata::new())
    };

    let catalog = update_catalog(source_file, catalog, translations);
    Ok(po_file::write(&catalog, output)?)
}
