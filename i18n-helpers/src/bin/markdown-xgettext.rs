use anyhow::{bail, Context};
use clap::{Arg, Command};
use mdbook_i18n_helpers::{add_message, extract_messages};
use polib::catalog::Catalog;
use polib::metadata::CatalogMetadata;
use polib::po_file;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{fs, io};

fn create_catalog(
    content: &str,
    source_path: &str,
    language: &str,
    title: Option<&str>,
) -> anyhow::Result<Catalog> {
    let mut metadata = CatalogMetadata::new();

    if let Some(title) = title {
        metadata.project_id_version = String::from(title);
    }

    metadata.language = String::from(language);
    let now = chrono::Local::now();
    metadata.pot_creation_date = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    metadata.mime_version = String::from("1.0");
    metadata.content_type = String::from("text/plain; charset=UTF-8");
    metadata.content_transfer_encoding = String::from("8bit");
    let mut catalog = Catalog::new(metadata);

    for (lineno, msgid) in extract_messages(content) {
        let source = format!("{}:{}", source_path, lineno);
        add_message(&mut catalog, &msgid, &source);
    }

    Ok(catalog)
}

fn ingest_input(input_dir: Option<&str>) -> anyhow::Result<(&str, String)> {
    if let Some(file_path) = input_dir {
        let content = fs::read_to_string(file_path)
            .context(format!("Failed to read from file {}", file_path))?;
        Ok((file_path, content))
    } else {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        Ok(("-", buffer))
    }
}

fn build_path(output_dir: Option<&str>, pot_title: &str) -> anyhow::Result<PathBuf> {
    let mut output_path = PathBuf::new();

    if let Some(out_dir) = output_dir {
        let out_dir_path = Path::new(&out_dir);

        if !out_dir_path.is_dir() {
            bail!("The specified output path is not a directory.");
        }
        output_path.push(out_dir_path);
    } else {
        output_path.push(".");
    }

    output_path.push(pot_title);

    Ok(output_path)
}

fn main() -> anyhow::Result<()> {
    let matches = Command::new("markdown-xgettext")
        .about("binary that extracts translatable text from markdown file for mdbook-i18n-helpers")
        .arg(Arg::new("input_dir").short('f').long("file"))
        .arg(Arg::new("language").short('l').long("lang").required(true))
        .arg(Arg::new("pot_title").short('t').long("title"))
        .arg(Arg::new("output_dir").short('o').long("out"))
        .get_matches();

    let (input, content) =
        ingest_input(matches.get_one::<String>("input_dir").map(|d| d.as_str()))?;
    let lang_option = matches.get_one::<String>("language").unwrap().as_str();
    let title = matches.get_one::<String>("pot_title").map(|t| t.as_str());

    let catalog = create_catalog(&content, input, lang_option, title)?;
    let output_path = build_path(
        matches.get_one::<String>("output_dir").map(|d| d.as_str()),
        title.unwrap_or("output.po"),
    )?;
    po_file::write(&catalog, output_path.as_path())
        .context(format!("Writing messages to {}", output_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    static MARKDOWN_CONTENT: &str = "# How to Foo from wef\n\
                                      \n\
                                      The first paragraph about Foo.\n\
                                      Still the first paragraph.\n";

    fn create_temp_file(content: &str) -> anyhow::Result<(TempDir, PathBuf)> {
        let tmp_dir = tempfile::tempdir()?;
        let file_path = tmp_dir.path().join("test_file.md");

        fs::write(&file_path, content)?;

        Ok((tmp_dir, file_path))
    }

    #[test]
    fn test_ingest_input_valid_file() -> anyhow::Result<()> {
        let (tmp_dir, tmp_path) = create_temp_file(MARKDOWN_CONTENT)?;

        let (input, content) = ingest_input(Some(tmp_path.to_str().unwrap()))?;

        assert_eq!(tmp_path.to_str().unwrap(), input);
        assert_eq!(content, MARKDOWN_CONTENT);

        tmp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_ingest_input_invalid_file() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let bogus_file_path = tmp_dir.path().join("bogus_file.md");

        let result = ingest_input(Some(bogus_file_path.to_str().unwrap()));
        assert!(result.is_err());

        tmp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_create_catalog_from_stdin() -> anyhow::Result<()> {
        let source_path = "-";
        let catalog = create_catalog(MARKDOWN_CONTENT, source_path, "fr", Some("wef"))?;

        for msg in catalog.messages() {
            assert!(!msg.is_translated());
        }

        assert_eq!(catalog.metadata.language, "fr");
        assert_eq!(catalog.metadata.project_id_version, "wef");
        assert_eq!(
            catalog
                .messages()
                .map(|msg| msg.msgid())
                .collect::<Vec<&str>>(),
            &[
                "How to Foo from wef",
                "The first paragraph about Foo. Still the first paragraph."
            ]
        );

        Ok(())
    }

    #[test]
    fn test_create_catalog_from_file() -> anyhow::Result<()> {
        let (tmp_dir, tmp_path) = create_temp_file(MARKDOWN_CONTENT)?;
        let (input, content) = ingest_input(Some(tmp_path.to_str().unwrap()))?;

        let catalog = create_catalog(&content, input, "en", None)?;

        for msg in catalog.messages() {
            assert!(!msg.is_translated());
        }

        assert_eq!(catalog.metadata.language, "en");

        assert_eq!(
            catalog
                .messages()
                .map(|msg| msg.msgid())
                .collect::<Vec<&str>>(),
            &[
                "How to Foo from wef",
                "The first paragraph about Foo. Still the first paragraph."
            ]
        );

        tmp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_build_path_given_valid_dir() -> anyhow::Result<()> {
        let tmp_dir = TempDir::new()?;
        let dir_path = tmp_dir.path().to_str().unwrap();

        let pot_title = "test.po";
        let result = build_path(Some(dir_path), pot_title)?;

        let mut expected = std::path::PathBuf::from(dir_path);
        expected.push(pot_title);
        assert_eq!(result, expected);

        tmp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_build_path_given_invalid_dir() {
        let invalid_dir = "/path/to/nonexistent/dir";
        let pot_title = "test.po";

        let result = build_path(Some(invalid_dir), pot_title);

        assert!(result.is_err())
    }

    #[test]
    fn test_build_path_default() -> anyhow::Result<()> {
        let pot_title = "test.po";
        let result = build_path(None, pot_title)?;

        let mut expected = std::path::PathBuf::from(".");
        expected.push(pot_title);
        assert_eq!(expected, result);
        Ok(())
    }
}
