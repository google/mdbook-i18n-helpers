use anyhow::{anyhow, bail, Context};
use mdbook_i18n_helpers::translate;
use polib::catalog::Catalog;
use polib::po_file;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::{env, fs};

fn split_argument(arg: &String) -> Option<String> {
    let splitted: Vec<_> = arg.split('=').collect();
    if splitted.len() == 2 {
        return Some(splitted[1].to_string());
    }

    None
}

fn build_catalog(lang_option: Option<String>) -> anyhow::Result<Catalog> {
    match lang_option {
        Some(lang_path) => {
            let pot_path = PathBuf::from(&lang_path);

            if pot_path.exists() {
                let catalog = po_file::parse(&pot_path)
                    .map_err(|err| anyhow!("{err}"))
                    .with_context(|| format!("Could not parse {:?} as PO file", pot_path))?;

                return Ok(catalog);
            }

            bail!("--po must be specified")
        }
        None => bail!("--po must be specified"),
    }
}

fn translate_files(
    catalog: Catalog,
    files: Vec<PathBuf>,
    output_path: PathBuf,
) -> anyhow::Result<()> {
    for file_path in files.iter() {
        let content = File::open(&file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;
        let mut buf_reader = BufReader::new(content);
        let mut contents = String::new();
        buf_reader
            .read_to_string(&mut contents)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let translated_content = translate(&contents, &catalog);

        // Prepare output file path
        let output_file_name = file_path.file_name().ok_or(anyhow!("Invalid file name"))?;
        let output_file_path = output_path.join(output_file_name);

        // Create and open the output file
        let mut output_file = File::create(&output_file_path)
            .with_context(|| format!("Failed to create file: {}", output_file_path.display()))?;

        // Write the translated content to the output file
        output_file
            .write_all(translated_content.as_bytes())
            .with_context(|| format!("Failed to write to file: {}", output_file_path.display()))?;
    }

    Ok(())
}

fn validate_file(path: &PathBuf) -> bool {
    path.is_file() && path.extension().map_or(false, |ext| ext == "md")
}

fn allocate_files(
    dir_option: Option<String>,
    file_option: Option<String>,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut valid_files = Vec::new();

    match (dir_option, file_option) {
        (Some(_), Some(_)) => return bail!("Only one of --dir or --f should be specified"),
        (None, None) => return bail!("Either --dir or --f must be specified"),
        (Some(dir_path), None) => {
            let full_dir = PathBuf::from(&dir_path);
            if !full_dir.exists() || !full_dir.is_dir() {
                return bail!("Directory does not exist: {}", full_dir.display());
            }

            for entry in fs::read_dir(full_dir)? {
                let entry = entry?;
                let path = entry.path();

                if validate_file(&path) {
                    valid_files.push(path);
                }
            }
        }
        (None, Some(file_path)) => {
            let full_file_path = PathBuf::from(&file_path);
            if !validate_file(&full_file_path) {
                bail!("Markdown file does not exist: {}", full_file_path.display())
            }
            valid_files.push(full_file_path);
        }
        _ => unreachable!(),
    }

    Ok(valid_files)
}

fn find_output_path(output_path_option: Option<String>) -> anyhow::Result<PathBuf> {
    match output_path_option {
        Some(output_path) => {
            let path = PathBuf::from(&output_path);
            if path.exists() {
                if !path.is_dir() {
                    bail!(
                        "Specified output path exists but is not a directory: {}",
                        path.display()
                    );
                }
            } else {
                fs::create_dir_all(&path)
                    .with_context(|| format!("Failed to create directory: {}", path.display()))?;
            }
            Ok(path)
        }
        None => {
            let default_path = PathBuf::from("translated_md_files");
            fs::create_dir_all(&default_path).with_context(|| {
                format!(
                    "Failed to create default directory: {}",
                    default_path.display()
                )
            })?;
            Ok(default_path)
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = env::args().collect::<Vec<_>>();

    let mut lang_option: Option<String> = None;
    let mut dir_option: Option<String> = None;
    let mut file_option: Option<String> = None;
    let mut output_path_option: Option<String> = None;

    let mut iter = args.iter().peekable();

    while let Some(arg) = iter.next() {
        if arg.starts_with("--po") {
            lang_option = split_argument(arg)
        } else if arg.starts_with("--dir") {
            dir_option = split_argument(arg)
        } else if arg.starts_with("--f") {
            file_option = split_argument(arg)
        } else if arg.starts_with("--out") {
            output_path_option = split_argument(arg)
        }
    }

    let files = allocate_files(dir_option, file_option)?;
    let catalog = build_catalog(lang_option)?;
    let output_path = find_output_path(output_path_option)?;

    translate_files(catalog, files, output_path)?;

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_argument() {
        assert_eq!(split_argument(&"--po=lang.po".to_string()), Some("lang.po".to_string()));
        assert_eq!(split_argument(&"--out src/resources".to_string()), None);
        assert_eq!(split_argument(&"--invalid".to_string()), None);
    }

    #[test]
    fn test_validate_file() -> anyhow::Result<()>{
        let tmp_dir = tempfile::tempdir()?;
        let md_path= tmp_dir.path().join("test.md");
        let html_path= tmp_dir.path().join("test.html");
        let source_path = md_path.to_str().unwrap();

        let markdown_content = "# How to Foo from a specified file path\n\
        \n\
        The first paragraph about Foo.\n\
        Still the first paragraph.*baz*\n";

        let html_content = "<h1>Hello Foo! </h1>";

        let mut md_file= File::create(&md_path)?;
        let mut html_file = File::create(&html_path)?;
        write!(md_file, "{}", markdown_content)?;
        write!(html_file, "{}", html_content)?;

        assert_eq!(validate_file(&md_path), true);
        assert_eq!(validate_file(&html_path), false);

        tmp_dir.close()?;

        Ok(())
    }


    // Unhappy path tests
    #[test]
    fn test_build_catalog_none() {
        let result = build_catalog(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_catalog_nonexistent_file() {
        let result = build_catalog(Some("nonexistent.po".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_allocate_files_neither_specified() {
        let result = allocate_files(None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_allocate_files_both_specified() {
        let result = allocate_files(Some("dir".to_string()), Some("file".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_find_output_path_not_a_directory() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let file_path = tmp_dir.path().join("file.txt");
        File::create(&file_path)?;

        let result = find_output_path(Some(file_path.to_str().unwrap().to_string()));
        assert!(result.is_err());

        tmp_dir.close()?;

        Ok(())
    }
}