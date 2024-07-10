# Translation status report generator

[![crates.io page](https://img.shields.io/crates/v/i18n-report.svg?style=flat-square)](https://crates.io/crates/i18n-report)

This is a utility to generate an HTML report from a set of PO files, showing the
current status of a set of translations.

## Installation

Run

```shell
$ cargo install i18n-report
```

## Usage

If your PO files are stored under `po/`:

```shell
$ i18n-report report.html po/*.po
```

## License

Licensed under the
[Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0).

---

This is not an officially supported Google product.
