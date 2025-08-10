use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Ok;
use clap::{Parser as _, arg};
use i18n_book_to_po::{
    catalog, file_map,
    structure::{align::align_markdown_docs, types::DiffAlgorithm},
};
use log::{info, warn};
use mdbook_i18n_helpers::extract_messages;

#[derive(clap::Parser)]
struct Cli {
    #[arg[short, long, value_name = "source/src"]]
    source: PathBuf,
    #[arg[short, long, value_name = "translation/src"]]
    translation: PathBuf,
    #[arg[short, long, value_name = "translation.po"]]
    output: PathBuf,
    #[arg[short, long, value_name = "diff_algorithm", value_enum, default_value_t = DiffAlgorithm::default()]]
    diff_algorithm: DiffAlgorithm,
}

///
/// create a translation file for a given source and translation file
///
/// This function takes the paths to a source markdown file, a translated
/// markdown file, and an output PO file. It aligns the source and translated
/// documents, extracts messages from the aligned documents, pairs them up,
/// and updates or creates a PO file with these translation pairs.
fn create_translation_for(
    source: &Path,
    translation: &Path,
    output: &Path,
    diff_algorithm: &DiffAlgorithm,
) -> anyhow::Result<()> {
    let source_content = fs::read_to_string(source)?;
    let translation_content = fs::read_to_string(translation)?;

    let (source_doc, translation_doc) =
        align_markdown_docs(&source_content, &translation_content, true, diff_algorithm)?;

    let source_messages = extract_messages(&source_doc);
    let translation_messages = extract_messages(&translation_doc);
    let translated_message_pairs = source_messages
        .unwrap()
        .into_iter()
        .zip(translation_messages.unwrap())
        .map(|((src_msg_id, src_msg), (_tr_msg_id, tr_msg))| {
            (src_msg_id, (src_msg.message, tr_msg.message))
        })
        .collect::<Vec<_>>();
    catalog::update_po_file(output, source, translated_message_pairs)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().filter_or("RUST_LOG", "info"));
    let cli = Cli::parse();
    info!("Reconstruct po file from translation of a book");
    let source = Path::new(&cli.source);
    let translation = Path::new(&cli.translation);
    let output = Path::new(&cli.output);
    let diff_algorithm = cli.diff_algorithm;

    let file_map = file_map::auto_folders_match(source, translation)?;

    for (source, translation) in &file_map {
        info!("Processing {}", source.display());
        if source.file_name() != translation.file_name() {
            warn!("filenames don't match")
        } else {
            create_translation_for(source, translation, output, &diff_algorithm)?;
        }
    }
    Ok(())
}
