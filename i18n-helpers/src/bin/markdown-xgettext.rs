use anyhow::{bail, Context};
use mdbook_i18n_helpers::{add_message, extract_messages};
use polib::catalog::Catalog;
use polib::metadata::CatalogMetadata;
use polib::po_file;
use std::io::Read;
use std::path::Path;
use std::{env, fs, io};

fn create_catalog(content: &str, source_path: &str, language: &str) -> anyhow::Result<Catalog> {
    let mut metadata = CatalogMetadata::new();

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

fn main() -> anyhow::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    let mut language = "en";

    for i in 0..args.len() {
        if args[i].starts_with("-lang") {
            if args[i] == "-lang" && i + 1 < args.len() {
                language = &args[i + 1];
            } else {
                let split: Vec<&str> = args[i].split('=').collect();
                if split.len() == 2 {
                    language = split[1];
                }
            }
        }
    }

    let (input, output) = match args.as_slice() {
        [_, input, output, ..] => (input, output),
        [prog_name, ..] => bail!(
            "Usage: {prog_name} <input.md> <output.po> [-lang=<language> OR -lang <language>]"
        ),
        [] => unreachable!(),
    };

    let content = if input == "-" {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        content
    } else {
        fs::read_to_string(input)?
    };

    let catalog = create_catalog(&content, input, language)?;

    po_file::write(&catalog, Path::new(output))
        .context(format!("Writing messages to {}", output))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Read, Write};

    #[test]
    fn test_create_catalog_from_stdin() -> anyhow::Result<()> {
        let markdown_content = "# How to Foo from Stdin\n\
                             \n\
                             The first paragraph about Foo from Stdin.\n\
                             Still the first paragraph from Stdin.\n";

        let source_path = "-";

        let catalog = create_catalog(markdown_content, source_path, "fr")?;

        for msg in catalog.messages() {
            assert!(!msg.is_translated());
        }

        assert_eq!(catalog.metadata.language, "fr");
        assert_eq!(
            catalog
                .messages()
                .map(|msg| msg.msgid())
                .collect::<Vec<&str>>(),
            &[
                "How to Foo from Stdin",
                "The first paragraph about Foo from Stdin. Still the first paragraph from Stdin."
            ]
        );

        Ok(())
    }

    #[test]
    fn test_create_catalog_from_file() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let file_path = tmp_dir.path().join("test.md");
        let source_path = file_path.to_str().unwrap();

        let markdown_content = "# How to Foo from a specified file path\n\
        \n\
        The first paragraph about Foo.\n\
        Still the first paragraph.*baz*\n";

        let mut tmp_file = File::create(&file_path)?;
        write!(tmp_file, "{}", markdown_content)?;

        let content = fs::read_to_string(&file_path)?;

        let catalog = create_catalog(&content, source_path, "en")?;

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
                "How to Foo from a specified file path",
                "The first paragraph about Foo. Still the first paragraph._baz_"
            ]
        );

        tmp_dir.close()?;

        Ok(())
    }
}
