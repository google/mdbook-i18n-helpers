use std::{
    fs,
    path::{Path, PathBuf},
};

use log::warn;

/// Try to map markdown files from the two given folders. Missing files will be ignored
pub fn auto_folders_match(
    source_base: &Path,
    translation_base: &Path,
) -> anyhow::Result<Vec<(PathBuf, PathBuf)>> {
    // discover all relevant files
    let mut source_filenames = discover_markdown_files(source_base)?;
    let translation_filenames = discover_markdown_files(translation_base)?;

    source_filenames.sort();

    // match the files according to their (identical) filenames
    let mut map = Vec::new();
    for source_file in source_filenames {
        // keep relative path inside the book as this should match
        let source_relative_path = source_file.strip_prefix(source_base)?;
        // try to find the same file in the translation files
        let translation_target_file = translation_base.join(source_relative_path);
        if translation_filenames.contains(&translation_target_file) {
            map.push((source_file, translation_target_file));
        } else {
            warn!(
                "no matching translation file found for '{}'",
                source_file.display()
            );
        }
    }
    Ok(map)
}

/// discover all markdown files in a given path
fn discover_markdown_files(path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension() == Some("md".as_ref()) {
            files.push(path);
        }
    }
    Ok(files)
}
