# Changelog

This lists the most important changes between releases.

## Version 0.2.4 (2023-09-27)

- [#87]: Set the POT-Creation-Date field in newly generated POT files.

## Version 0.2.3 (2023-09-19)

- [#75]: Automatically ignore code blocks without string literals and line
  comments.
- [#69]: Add support for skipping the next translation group.

## Version 0.2.2 (2023-08-23)

- [#59]: Resolve broken links using the original sources.

## Version 0.2.1 (2023-08-15)

- [#56]: Handle normalization where old `msgid` disappears.

## Version 0.2.0 (2023-08-15)

> This is a breaking release. Please make sure to
> [run `mdbook-i18n-normalize` on your existing PO files](i18n-helpers/USAGE.md)!

- [#49]: Link to other projects which use mdbook-i18n-helpers.
- [#46]: Add `mdbook-i18n-normalize` to convert existing PO files.
- [#27]: Normalize soft breaks to space.
- [#25]: Implement fine-grained extraction of translatable text.

## Version 0.1.0 (2023-04-05)

First release as a stand-alone crate.

[#87]: https://github.com/google/mdbook-i18n-helpers/pull/87
[#75]: https://github.com/google/mdbook-i18n-helpers/pull/75
[#69]: https://github.com/google/mdbook-i18n-helpers/pull/69
[#59]: https://github.com/google/mdbook-i18n-helpers/pull/59
[#56]: https://github.com/google/mdbook-i18n-helpers/pull/56
[#49]: https://github.com/google/mdbook-i18n-helpers/pull/49
[#46]: https://github.com/google/mdbook-i18n-helpers/pull/46
[#27]: https://github.com/google/mdbook-i18n-helpers/pull/27
[#25]: https://github.com/google/mdbook-i18n-helpers/pull/25
