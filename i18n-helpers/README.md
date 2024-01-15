# Gettext Translation Support for `mdbook`

[![Visit crates.io](https://img.shields.io/crates/v/mdbook-i18n-helpers?style=flat-square)](https://crates.io/crates/mdbook-i18n-helpers)
[![Build workflow](https://img.shields.io/github/actions/workflow/status/google/mdbook-i18n-helpers/test.yml?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/actions/workflows/test.yml?query=branch%3Amain)
[![GitHub contributors](https://img.shields.io/github/contributors/google/mdbook-i18n-helpers?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/graphs/contributors)
[![GitHub stars](https://img.shields.io/github/stars/google/mdbook-i18n-helpers?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/stargazers)

The plugins here makes it easy to translate documentation written in
[`mdbook`](https://github.com/rust-lang/mdBook/) into multiple languages.
Support for translations is a
[long-stading feature request for `mdbook`](https://github.com/rust-lang/mdBook/issues/5).

## Installation

Run

```shell
$ cargo install mdbook-i18n-helpers
```

Please see [USAGE](USAGE.md) for how to translate your `mdbook` project.

## Configuration

You can customize the plugins in your `book.toml` file.

### `mdbook-xgettext`

- `output.xgettext.pot-file` (no default, required): Set this to the path of
  your POT file. A typical value is `messages.pot`, see examples in
  [USAGE](USAGE.md).
- `output.xgettext.granularity` (default: `1`): Set this to _n_ to round all
  line numbers down to the nearest multiple of _n_. Set this to `0` to remove
  line numbers completely from the PO file. This can help reduce churn in your
  PO files when messages edited.

## Changelog

Please see the [CHANGELOG](CHANGELOG.md) for details on the major changes in
each release.

## Contact

For questions or comments, please contact
[Martin Geisler](mailto:mgeisler@google.com) or start a
[discussion](https://github.com/google/mdbook-i18n-helpers/discussions). We
would love to hear from you.

---

This is not an officially supported Google product.
