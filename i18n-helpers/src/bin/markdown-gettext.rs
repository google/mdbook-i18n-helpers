use anyhow::{anyhow, bail, Context};
use clap::{Arg, ArgGroup, Command};
use mdbook_i18n_helpers::translate;
use polib::catalog::Catalog;
use polib::po_file;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

fn build_catalog(lang: &str) -> anyhow::Result<Catalog> {
    let pot_path = Path::new(lang);
    let catalog = po_file::parse(pot_path)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("Could not parse {:?} as PO file", pot_path))?;

    Ok(catalog)
}

fn translate_files(
    catalog: Catalog,
    files: Vec<PathBuf>,
    output_path: PathBuf,
) -> anyhow::Result<()> {
    for file_path in files.iter() {
        let content = File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;
        let mut buf_reader = BufReader::new(content);
        let mut contents = String::new();
        buf_reader
            .read_to_string(&mut contents)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let translated_content = translate(&contents, &catalog);

        let output_file_name = file_path.file_name().ok_or(anyhow!("Invalid file name"))?;
        let output_file_path = output_path.join(output_file_name);

        let mut output_file = File::create(&output_file_path)
            .with_context(|| format!("Failed to create file: {}", output_file_path.display()))?;

        output_file
            .write_all(translated_content.as_bytes())
            .with_context(|| format!("Failed to write to file: {}", output_file_path.display()))?;
    }
    Ok(())
}

fn validate_file(path: &Path) -> bool {
    path.is_file() && path.extension().map_or(false, |ext| ext == "md")
}

fn allocate_files(
    dir_option: Option<&str>,
    file_option: Option<&str>,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut valid_files = Vec::new();
    match (dir_option, file_option) {
        (Some(dir_path), None) => {
            let full_dir = PathBuf::from(dir_path);
            fs::read_dir(full_dir)?
                .filter_map(Result::ok)
                .filter(|entry| validate_file(&entry.path()))
                .for_each(|entry| valid_files.push(entry.path()));
        }
        (None, Some(file_path)) => {
            let full_file_path = PathBuf::from(file_path);
            if !validate_file(&full_file_path) {
                bail!("Markdown file does not exist: {}", full_file_path.display())
            }
            valid_files.push(full_file_path);
        }
        _ => unreachable!(),
    }
    Ok(valid_files)
}

fn build_output_path(output_path_option: Option<&str>) -> anyhow::Result<PathBuf> {
    let output_path = output_path_option.unwrap_or("translated_md_files");
    let path = PathBuf::from(output_path);
    fs::create_dir_all(&path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    Ok(path)
}

fn main() -> anyhow::Result<()> {
    let matches = Command::new("markdown-gettext")
        .about("markdown translator binary for mdbook-i18n-helpers")
        .arg(Arg::new("input_dir").short('d').long("dir"))
        .arg(Arg::new("file").short('f').long("file"))
        .arg(Arg::new("po").short('p').long("po").required(true))
        .arg(Arg::new("output_dir").short('o').long("out"))
        .group(
            ArgGroup::new("input_source")
                .args(["input_dir", "file"])
                .required(true)
                .multiple(false),
        )
        .get_matches();

    let lang = matches.get_one::<String>("po").unwrap().as_str();
    let dir_option = matches.get_one::<String>("input_dir").map(|d| d.as_str());
    let file_option = matches.get_one::<String>("file").map(|f| f.as_str());
    let output_path_option = matches.get_one::<String>("output_dir").map(|d| d.as_str());

    let files = allocate_files(dir_option, file_option)?;
    let catalog = build_catalog(lang)?;
    let output_path = build_output_path(output_path_option)?;

    translate_files(catalog, files, output_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    static MARKDOWN_CONTENT: &str = "# How to Foo from a specified file path\n\
        \n\
        The first paragraph about Foo.\n\
        Still the first paragraph.*baz*\n";

    static INVALID_CONTENT: &str = "<p> This is not markdown content </p>";

    fn create_temp_directory(
        content: &str,
        file_title: &str,
    ) -> anyhow::Result<(TempDir, PathBuf)> {
        let tmp_dir = tempfile::tempdir()?;
        let file_path = tmp_dir.path().join(file_title);

        fs::write(&file_path, content)?;

        Ok((tmp_dir, file_path))
    }

    #[test]
    fn test_allocate_files_with_dir() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        fs::write(tmp_dir.path().join("valid.md"), MARKDOWN_CONTENT)?;
        fs::write(tmp_dir.path().join("invalid.html"), INVALID_CONTENT)?;
        fs::write(tmp_dir.path().join("ibid.txt"), INVALID_CONTENT)?;

        let valid_files = allocate_files(Some(tmp_dir.path().to_str().unwrap()), None)?;
        assert_eq!(valid_files.len(), 1);
        tmp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_allocate_files_valid_file() -> anyhow::Result<()> {
        let (temp_dir, valid_file) = create_temp_directory(MARKDOWN_CONTENT, "test.md")?;
        let valid_files = allocate_files(None, Some(valid_file.to_str().unwrap()))?;
        assert_eq!(valid_files.len(), 1);

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_allocate_files_invalid_file() -> anyhow::Result<()> {
        let (tmp_dir, invalid_file) = create_temp_directory(INVALID_CONTENT, "wef.html")?;
        let result = allocate_files(None, Some(invalid_file.to_str().unwrap()));

        assert!(result.is_err());

        tmp_dir.close()?;

        Ok(())
    }

    #[test]
    fn test_validate_file() -> anyhow::Result<()> {
        let (md_dir, md_path) = create_temp_directory(MARKDOWN_CONTENT, "test.md")?;
        let (html_dir, html_path) = create_temp_directory(INVALID_CONTENT, "test.html")?;

        assert!(validate_file(&md_path));
        assert!(!validate_file(&html_path));

        md_dir.close()?;
        html_dir.close()?;

        Ok(())
    }

    #[test]
    fn test_build_catalog_nonexistent_file() {
        let result = build_catalog("nonexistent.po");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_output_path_not_a_directory() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let file_path = tmp_dir.path().join("wef.txt");
        File::create(&file_path)?;

        let result = build_output_path(Some(&file_path.to_str().unwrap().to_string()));
        assert!(result.is_err());

        tmp_dir.close()?;

        Ok(())
    }

    #[test]
    fn test_find_output_path_default() -> anyhow::Result<()> {
        let default_output_path = build_output_path(None)?;
        let dir_output_path = build_output_path(Some("wef_dir"))?;

        assert_eq!(default_output_path, PathBuf::from("translated_md_files"));
        assert_eq!(dir_output_path, PathBuf::from("wef_dir"));
        assert!(default_output_path.is_dir() && dir_output_path.is_dir());

        fs::remove_dir_all("translated_md_files")?;
        fs::remove_dir_all("wef_dir")?;

        Ok(())
    }

    #[test]
    fn test_find_output_path_given_existing_dir() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let output_path = build_output_path(Some(tmp_dir.path().to_str().unwrap()))?;
        assert_eq!(output_path, tmp_dir.path());
        assert!(output_path.is_dir());

        tmp_dir.close()?;

        Ok(())
    }

    #[test]
    fn test_find_output_path_given_invalid() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let tmp_file = tmp_dir.path().join("temp.md");
        File::create(&tmp_file)?;

        let file_result = build_output_path(Some(tmp_file.to_str().unwrap()));
        let rogue_result = build_output_path(Some("\0"));

        assert!(file_result.is_err() && rogue_result.is_err());

        tmp_dir.close()?;

        Ok(())
    }
}
