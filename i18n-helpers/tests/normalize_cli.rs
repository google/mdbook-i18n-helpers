// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use polib::po_file;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

/// Run `mdbook-i18n-normalize` with `directory` as working directory.
fn run(directory: &Path, args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mdbook-i18n-normalize"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// A catalog with a single message which normalizes to two messages.
const CATALOG: &str = r##"msgid ""
msgstr ""
"Language: fr\n"
"Content-Type: text/plain; charset=UTF-8\n"

#: src/foo.md:1
msgid ""
"# Heading\n"
"\n"
"First\n"
"paragraph."
msgstr ""
"# TITRE\n"
"\n"
"PREMIER\n"
"PARAGRAPHE."
"##;

#[test]
fn normalize_writes_normalized_catalog() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.po");
    let output = directory.path().join("output.po");
    fs::write(&input, CATALOG).unwrap();

    let result = run(directory.path(), &[&input, &output]);
    assert!(result.status.success(), "{result:?}");
    assert!(result.stdout.is_empty(), "{result:?}");
    assert!(result.stderr.is_empty(), "{result:?}");

    let catalog = po_file::parse(&output).unwrap();
    assert_eq!(catalog.metadata.language, "fr");
    let messages = catalog
        .messages()
        .map(|message| (message.source(), message.msgid(), message.msgstr().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        &[
            ("src/foo.md:1", "Heading", "TITRE"),
            ("src/foo.md:3", "First paragraph.", "PREMIER PARAGRAPHE."),
        ]
    );
    // The input file is left untouched.
    assert_eq!(fs::read_to_string(&input).unwrap(), CATALOG);
}

#[test]
fn wrong_number_of_arguments_prints_usage() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.po");
    let output = directory.path().join("output.po");
    let extra = directory.path().join("extra.po");
    fs::write(&input, CATALOG).unwrap();

    let cases: [&[&Path]; 3] = [&[], &[&input], &[&input, &output, &extra]];
    for args in cases {
        let error = stderr(&run(directory.path(), args));
        assert!(error.contains("Usage: "), "{error}");
        assert!(error.contains("<input.po> <output.po>"), "{error}");
        assert!(!output.exists());
    }
}

#[test]
fn missing_input_error_names_the_input_file() {
    let directory = tempdir().unwrap();
    let missing = directory.path().join("missing.po");
    let output = directory.path().join("output.po");

    let error = stderr(&run(directory.path(), &[&missing, &output]));
    assert!(error.contains("Could not parse"), "{error}");
    assert!(error.contains("missing.po"), "{error}");
    assert!(!error.contains("output.po"), "{error}");
    assert!(!output.exists());
}

#[test]
fn output_write_error_names_the_output_file() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.po");
    let output = directory.path().join("output-directory");
    fs::write(&input, CATALOG).unwrap();
    fs::create_dir(&output).unwrap();

    let error = stderr(&run(directory.path(), &[&input, &output]));
    assert!(error.contains("Could not write catalog to"), "{error}");
    assert!(error.contains("output-directory"), "{error}");
}
