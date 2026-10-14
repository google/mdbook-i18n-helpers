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
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::tempdir;

fn run(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_i18n-report"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn write_catalog(directory: &Path, filename: &str, language: &str, translation: &str) -> PathBuf {
    let path = directory.join(filename);
    fs::write(
        &path,
        format!(
            r#"msgid ""
msgstr ""
"Language: {language}\n"
"Content-Type: text/plain; charset=UTF-8\n"
"POT-Creation-Date: 2026-01-02 03:04+0000\n"

msgid "Hello"
msgstr "{translation}"
"#
        ),
    )
    .unwrap();
    path
}

#[test]
fn report_writes_language_rows_sorted_by_translated_count() {
    let directory = tempdir().unwrap();
    let german = write_catalog(directory.path(), "de.po", "de", "");
    let french = write_catalog(directory.path(), "fr.po", "fr", "Bonjour");
    let report = directory.path().join("report.html");

    assert_eq!(
        stdout(&run(&[Path::new("report"), &report, &german, &french])),
        ""
    );
    let html = fs::read_to_string(report).unwrap();
    let html = html.split_whitespace().collect::<Vec<_>>().join(" ");
    let french_row = "<td>fr</td> <td>2026-01-02 03:04+0000</td> <td>1</td> <td>1</td> <td>0</td> <td>0</td> <td>0</td>";
    let german_row = "<td>de</td> <td>2026-01-02 03:04+0000</td> <td>1</td> <td>0</td> <td>0</td> <td>1</td> <td>0</td>";
    assert!(html.find(french_row).unwrap() < html.find(german_row).unwrap());
    assert!(html.contains("class=\"bar translated\" style=\"width: 100.0%\""));
}

#[test]
fn report_without_catalogs_writes_an_empty_table() {
    let directory = tempdir().unwrap();
    let report = directory.path().join("report.html");

    assert_eq!(stdout(&run(&[Path::new("report"), &report])), "");
    let html = fs::read_to_string(report).unwrap();
    assert!(html.contains("<th>Language</th>"));
    assert!(!html.contains("<td>"));
}

#[test]
fn report_propagates_missing_catalog_error() {
    let directory = tempdir().unwrap();
    let missing = directory.path().join("missing.po");
    let report = directory.path().join("report.html");

    let output = run(&[Path::new("report"), &report, &missing]);
    assert!(!output.status.success(), "{output:?}");
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Could not parse"), "{error}");
    assert!(error.contains("missing.po"), "{error}");
    assert!(!report.exists());
}

#[test]
fn report_propagates_output_write_error() {
    let directory = tempdir().unwrap();
    let catalog = write_catalog(directory.path(), "fr.po", "fr", "Bonjour");

    let output = run(&[Path::new("report"), directory.path(), &catalog]);
    assert!(!output.status.success(), "{output:?}");
    assert!(!output.stderr.is_empty());
}

#[test]
fn diff_reports_added_removed_and_changed_languages_in_order() {
    let directory = tempdir().unwrap();
    let old = directory.path().join("old");
    let new = directory.path().join("new");
    fs::create_dir(&old).unwrap();
    fs::create_dir(&new).unwrap();
    // Filenames deliberately sort differently from their language metadata.
    write_catalog(&old, "a.po", "fr", "");
    write_catalog(&old, "z.po", "de", "Hallo");
    write_catalog(&new, "a.po", "fr", "Bonjour");
    write_catalog(&new, "z.po", "es", "Hola");
    fs::write(new.join("ignored.txt"), "not a PO file").unwrap();
    fs::write(new.join("ignored.pot"), "not a PO file").unwrap();
    fs::create_dir(new.join("ignored-directory")).unwrap();

    assert_eq!(
        stdout(&run(&[Path::new("diff"), &old, &new])),
        concat!(
            "Counts are \"translated (fuzzy, fuzzy untranslated) / total\"\n",
            "Removed de: 1 (0, 0) / 1, creation date 2026-01-02 03:04+0000\n",
            "Added es: 1 (0, 0) / 1, creation date 2026-01-02 03:04+0000\n",
            "Changed fr: 0 (0, 0) / 1, creation date 2026-01-02 03:04+0000 -> fr: 1 (0, 0) / 1, creation date 2026-01-02 03:04+0000\n",
        ),
    );
}

#[test]
fn diff_reports_languages_remaining_after_one_side_is_exhausted() {
    let directory = tempdir().unwrap();
    let empty = directory.path().join("empty");
    let populated = directory.path().join("populated");
    fs::create_dir(&empty).unwrap();
    fs::create_dir(&populated).unwrap();
    write_catalog(&populated, "a.po", "fr", "Bonjour");
    write_catalog(&populated, "z.po", "de", "Hallo");

    for (old, new, action) in [
        (&empty, &populated, "Added"),
        (&populated, &empty, "Removed"),
    ] {
        assert_eq!(
            stdout(&run(&[Path::new("diff"), old, new])),
            format!(
                "Counts are \"translated (fuzzy, fuzzy untranslated) / total\"\n\
                 {action} de: 1 (0, 0) / 1, creation date 2026-01-02 03:04+0000\n\
                 {action} fr: 1 (0, 0) / 1, creation date 2026-01-02 03:04+0000\n"
            ),
        );
    }
}

#[test]
fn diff_is_silent_for_identical_catalogs_with_different_filenames() {
    let directory = tempdir().unwrap();
    let old = directory.path().join("old");
    let new = directory.path().join("new");
    fs::create_dir(&old).unwrap();
    fs::create_dir(&new).unwrap();
    write_catalog(&old, "a.po", "fr", "Bonjour");
    write_catalog(&old, "z.po", "de", "Hallo");
    write_catalog(&new, "z.po", "fr", "Bonjour");
    write_catalog(&new, "a.po", "de", "Hallo");

    assert_eq!(stdout(&run(&[Path::new("diff"), &old, &new])), "");
}

#[test]
fn diff_propagates_missing_directory_error() {
    let directory = tempdir().unwrap();
    let missing = directory.path().join("missing");

    for (old, new) in [
        (missing.as_path(), directory.path()),
        (directory.path(), missing.as_path()),
    ] {
        let output = run(&[Path::new("diff"), old, new]);
        assert!(!output.status.success(), "{output:?}");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn invalid_command_fails() {
    let output = run(&[Path::new("not-a-command")]);
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("not-a-command"));
}
