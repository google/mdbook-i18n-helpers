//! Normalize the Markdown in a  a PO or POT file.
//!
//! This program will process all entries in a PO or POT file and
//! normalize the Markdown found there. Both the `msgid` (the source
//! text) and the `msgstr` (the translated text, if any) fields will
//! be normalized.
//!
//! The result is as if you extract the Markdown anew with the current
//! version of the `mdbook-xgettext` renderer. This allows you to
//! safely move to a new version of the mdbook-i18n-helpers without
//! losing existing translations.

use std::path::Path;

use anyhow::{bail, Context};
use mdbook_i18n_helpers::normalize::normalize;
use polib::po_file;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let [input, output] = match args.as_slice() {
        [_, input, output] => [input, output],
        [prog_name, ..] => bail!("Usage: {prog_name} <input.po> <output.po>"),
        [] => unreachable!(),
    };

    let catalog = po_file::parse(Path::new(input))
        .with_context(|| format!("Could not parse {:?}", &output))?;
    let normalized = normalize(catalog)?;
    po_file::write(&normalized, Path::new(output))
        .with_context(|| format!("Could not write catalog to {}", &output))?;

    Ok(())
}
