# Changelog

This lists the most important changes between releases of mdbook-i18n-helpers.

## Version 0.3.3 (2024-05-25)

- [#107], [#177]: Support for translator comments by @dyoo
- [#147]: Performance optimization with caching `SyntaxSet` by @mgeisler
- [#143], [#149], [#162], [#168], [#179]: Stability improvements to coverage
  reports by @kdarkhan
- [#153], [#157]: Fix wrong codeblock indent count by @dalance
- [#155]: `fuzz` directory was made a part of the workspace by @kdarkhan
- [#156]: Fix for duplicate sources in PO files by @zachcmadsen
- [#193]: Export `Gettext` preprocessor to enable the lib to be used in Rust
  compiler docs by @dalance
- [#195]: Improve grouping behavior of inline HTML by @michael-kerscher

## Version 0.3.2 (2024-01-15)

- [#145]: Add support for rounding line numbers by @mgeisler.

## Version 0.3.1 (2024-01-05)

- [#129]: Fix nested codeblocks by @dalance.
- [#121]: Wrap source lines in `mdbook-i18n-normalize` by @mgeisler.
- [#128]: Define fuzzer for `xgettext` binary by @kdarkhan.

## Version 0.3.0 (2023-11-09)

This release changes how code blocks are treated: we now only extract literal
strings and comments. Other parts of the code block is ignored. This vastly
improves the experience when translating books with many code samples. We will
add [more controls][#76] for this in the future.

> This is a breaking change: if you translate strings and comments in your code
> blocks, then you should [run `mdbook-i18n-normalize`](USAGE.md) to migrate
> them automatically!

- [#111]: Skip extracting whitespace-only messages.
- [#109]: Extract only string literals and comments from code blocks.
- [#100]: Allow formatting in the `SUMMARY.md` file.
- [#93]: Wrap the source lines like `msgmerge` does.

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
> [run `mdbook-i18n-normalize` on your existing PO files](USAGE.md)!

- [#49]: Link to other projects which use mdbook-i18n-helpers.
- [#46]: Add `mdbook-i18n-normalize` to convert existing PO files.
- [#27]: Normalize soft breaks to space.
- [#25]: Implement fine-grained extraction of translatable text.

## Version 0.1.0 (2023-04-05)

First release as a stand-alone crate.

[#195]: https://github.com/google/mdbook-i18n-helpers/pull/195
[#193]: https://github.com/google/mdbook-i18n-helpers/pull/193
[#179]: https://github.com/google/mdbook-i18n-helpers/pull/179
[#177]: https://github.com/google/mdbook-i18n-helpers/pull/177
[#168]: https://github.com/google/mdbook-i18n-helpers/pull/168
[#162]: https://github.com/google/mdbook-i18n-helpers/pull/162
[#157]: https://github.com/google/mdbook-i18n-helpers/pull/157
[#156]: https://github.com/google/mdbook-i18n-helpers/pull/156
[#155]: https://github.com/google/mdbook-i18n-helpers/pull/155
[#153]: https://github.com/google/mdbook-i18n-helpers/pull/153
[#149]: https://github.com/google/mdbook-i18n-helpers/pull/149
[#147]: https://github.com/google/mdbook-i18n-helpers/pull/147
[#145]: https://github.com/google/mdbook-i18n-helpers/pull/145
[#143]: https://github.com/google/mdbook-i18n-helpers/pull/143
[#129]: https://github.com/google/mdbook-i18n-helpers/pull/129
[#128]: https://github.com/google/mdbook-i18n-helpers/pull/128
[#121]: https://github.com/google/mdbook-i18n-helpers/pull/121
[#111]: https://github.com/google/mdbook-i18n-helpers/pull/111
[#109]: https://github.com/google/mdbook-i18n-helpers/pull/109
[#107]: https://github.com/google/mdbook-i18n-helpers/pull/107
[#100]: https://github.com/google/mdbook-i18n-helpers/pull/100
[#93]: https://github.com/google/mdbook-i18n-helpers/pull/93
[#87]: https://github.com/google/mdbook-i18n-helpers/pull/87
[#76]: https://github.com/google/mdbook-i18n-helpers/issues/76
[#75]: https://github.com/google/mdbook-i18n-helpers/pull/75
[#69]: https://github.com/google/mdbook-i18n-helpers/pull/69
[#59]: https://github.com/google/mdbook-i18n-helpers/pull/59
[#56]: https://github.com/google/mdbook-i18n-helpers/pull/56
[#49]: https://github.com/google/mdbook-i18n-helpers/pull/49
[#46]: https://github.com/google/mdbook-i18n-helpers/pull/46
[#27]: https://github.com/google/mdbook-i18n-helpers/pull/27
[#25]: https://github.com/google/mdbook-i18n-helpers/pull/25
