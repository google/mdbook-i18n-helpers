# Internationalization and Rendering extensions for `mdbook`

[![Visit crates.io](https://img.shields.io/crates/v/mdbook-i18n-helpers?style=flat-square)](https://crates.io/crates/mdbook-i18n-helpers)
[![Build workflow](https://img.shields.io/github/actions/workflow/status/google/mdbook-i18n-helpers/test.yml?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/actions/workflows/test.yml?query=branch%3Amain)
[![GitHub contributors](https://img.shields.io/github/contributors/google/mdbook-i18n-helpers?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/graphs/contributors)
[![GitHub stars](https://img.shields.io/github/stars/google/mdbook-i18n-helpers?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/stargazers)

This repository contains the following crates that provide extensions and
infrastructure for [mdbook](https://github.com/rust-lang/mdBook/):

- [mdbook-i18n-helpers](./i18n-helpers/README.md): Gettext translation support
  for [mdbook](https://github.com/rust-lang/mdBook/)
- [mdbook-tera-backend](./mdbook-tera-backend/README.md): Tera templates
  extension for [mdbook](https://github.com/rust-lang/mdBook/)'s HTML renderer.
- [i18n-report](i18n-report/README.md): A tool to generate an HTML report
  comparing the status of a number of translations.

## Showcases

### mdbook-i18n-helpers

Please add your project below if it uses
[mdbook-i18n-helpers](i18n-helpers/README.md) for translations:

- [Comprehensive Rust 🦀](https://google.github.io/comprehensive-rust/)
- [Game Boy Assembly Tutorial](https://gbdev.io/gb-asm-tutorial/)
- [Ordinal Theory Handbook](https://docs.ordinals.com/)
- [Getting Started with SONiC](https://r12f.com/sonic-book/)
- [Dojo: The Provable Game Engine](https://book.dojoengine.org/)
- [ezlog documentation](https://s1rius.github.io/ezlog/)
- [The Cairo Programming Language](https://book.cairo-lang.org/)
- [The Veryl Hardware Description Language](https://doc.veryl-lang.org/book/)
- [Rust By Example](https://github.com/rust-lang/rust-by-example)
- [Localization of Comprehensive Rust](https://github.com/localizethedocs/comprehensive-rust-l10n)
- [Localization of The Asterinas Book](https://github.com/localizethedocs/asterinas-book-l10n)
- [Localization of The Cargo Book](https://github.com/localizethedocs/cargo-book-l10n)
- [Localization of The Helix Documentation](https://github.com/localizethedocs/helix-docs-l10n)
- [Localization of The Leptos Book](https://github.com/localizethedocs/leptos-book-l10n)
- [Localization of The Maturin User Guide](https://github.com/localizethedocs/maturin-guide-l10n)
- [Localization of The mdBook Documentation](https://github.com/localizethedocs/mdbook-docs-l10n)
- [Localization of The PRQL Language Book](https://github.com/localizethedocs/prql-book-l10n)
- [Localization of The PyO3 User Guide](https://github.com/localizethedocs/pyo3-guide-l10n)
- [Localization of The Redox OS Book](https://github.com/localizethedocs/redoxos-book-l10n)
- [Localization of The Rust Book](https://github.com/localizethedocs/rust-book-l10n)
- [Localization of The Rust Edition Guide](https://github.com/localizethedocs/rust-edition-guide-l10n)
- [Localization of The Rust Embedded Book](https://github.com/localizethedocs/rust-embedded-book-l10n)
- [Localization of The Rust Design Patterns](https://github.com/localizethedocs/rust-patterns-l10n)
- [Localization of The Rust RFCs Book](https://github.com/localizethedocs/rust-rfcs-l10n)
- [Localization of The Rustdoc Book](https://github.com/localizethedocs/rustdoc-book-l10n)
- [Localization of The Zed Documentation](https://github.com/localizethedocs/zed-docs-l10n)

### i18n-report

- [Comprehensive Rust 🦀](https://google.github.io/comprehensive-rust/translation-report.html)

## Installation

### `mdbook-i18n-helpers`

Run

```shell
cargo install mdbook-i18n-helpers
```

Please see [USAGE](i18n-helpers/USAGE.md) for how to translate your
[mdbook](https://github.com/rust-lang/mdBook/) project.

Please see the [i18n-helpers/CHANGELOG](CHANGELOG) for details on the changes in
each release.

### `mdbook-tera-backend`

Run

```shell
cargo install mdbook-tera-backend
```

### `i18n-report`

Run

```shell
cargo install i18n-report
```

## Contact

For questions or comments, please contact
[Martin Geisler](mailto:mgeisler@google.com) or start a
[discussion](https://github.com/google/mdbook-i18n-helpers/discussions). We
would love to hear from you.

---

This is not an officially supported Google product.
