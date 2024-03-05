# Usage

How to use the translation infrastructure with your `mdbook` project.

## Installation

Run

```shell
cargo install mdbook-i18n-helpers
```

to install three binaries:

- `mdbook-xgettext`: This program extracts the source text. It is an
  [`mdbook` renderer].
- `mdbook-gettext`: This program translates the book into a target language. It
  is an [`mdbook` preprocessor].
- `mdbook-i18n-normalize`: This program normalizs a PO file. Use it after
  breaking changes.

[`mdbook` renderer]: https://rust-lang.github.io/mdBook/format/configuration/renderers.html
[`mdbook` preprocessor]: https://rust-lang.github.io/mdBook/format/configuration/preprocessors.html

Together, the two programs makes it possible to do i18n for `mdbook` in a
standard and maintainable way.

## Gettext Overview

We use the [Gettext] system for translations. This system is widely used for
translations of open source software and it also works reasonably well for
documentation.

The advantage of Gettext is that you get a structured way to approach the
translations. Instead of copying Markdown files and tracking changes by hand,
you modify `.po` files in a `po/` directory. The `.po` files are small
text-based translation databases. You update the `.po` files using tools
(described below) and you can see at a glance how much text still needs to be
translated.

> **Tip:** You should never edit the `.po` files by hand. Instead use a PO
> editor, such as [Poedit](https://poedit.net/). There are also several online
> editors available. This will ensure that the file is encoded correctly.

There is a `.po` file for each language. They are named after the [ISO 639]
language codes: Danish would go into `po/da.po`, Korean would go into
`po/ko.po`, etc. The `.po` files contain all the source text plus the
translations. They are initialized from a `messages.pot` file (a PO template)
which contains the extracted source text from your `mdbook` project.

If your source files are in English, then the `messages.pot` file will contain
the English text and your translators will be translating from English into
their target language.

We will show how to update and manipulate the `.po` and `.pot` files using the
GNU Gettext utilities below.

[Gettext]: https://www.gnu.org/software/gettext/manual/html_node/index.html
[ISO 639]: https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes

## Creating and Updating Translations

First, you need to know how to update the `.pot` and `.po` files.

As a general rule, you should never touch the auto-generated `po/messages.pot`
file. You should not even check it into your repository since it can be fully
generated from your source Markdown files.

You should also never edit the `msgid` entries in a `po/xx.po` file. If you find
mistakes, you need to update the original text instead. The fixes to the
original text will flow into the `.po` files the next time the translators
update them.

### Generating the PO Template

To extract the original text and generate a `messages.pot` file, you run
`mdbook` with the `mdbook-xgettext` renderer:

```shell
MDBOOK_OUTPUT='{"xgettext": {}}' \
  mdbook build -d po
```

You will find the generated POT file as `po/messages.pot`.

To extract the text into smaller `.pot` files based on the text's Markdown
outline, use the `depth` parameter. For a `depth` of `1`, the `.pot` lines will
be separated into a file for each section or chapter title. Use greater values
to split the `.pot` file further.

```shell
MDBOOK_OUTPUT='{"xgettext": {"depth": "1"}}' \
  mdbook build -d po/messages
```

### Initialize a New Translation

To start a new translation for a fictional `xx` locale, first generate the
`po/messages.pot` file. Then use `msginit` to create a `xx.po` file:

```shell
msginit -i po/messages.pot -l xx -o po/xx.po
```

You can also simply copy `po/messages.pot` to `po/xx.po` if you don't have
`msginit` from the GNU Gettext tools available. If you do that, then you have to
update the header (the first entry with `msgid ""`) manually to the correct
language.

> **Tip:** You can use the
> [`cloud-translate`](https://github.com/mgeisler/cloud-translate) tool to
> quickly machine-translate a new translation. Untranslated entries will be sent
> through GCP Cloud Translate. Some of the translations will be wrong after
> this, so you must inspect them by hand afterwards.

### Updating an Existing Translation

As the source text changes, translations gradually become outdated. To update
the `po/xx.po` file with new messages, first extract the source text into a
`po/messages.pot` template file. Then run

```shell
msgmerge --update po/xx.po po/messages.pot
```

Unchanged messages will stay intact, deleted messages are marked as old, and
updated messages are marked "fuzzy". A fuzzy entry will reuse the previous
translation: you should then go over it and update it as necessary before you
remove the fuzzy marker.

## Using Translations

This will show you how to use the translations to generate localized HTML
output.

> **Note:** `mdbook-gettext` will use the original untranslated text for all
> entries marked as "fuzzy" (visible as "Needs work" in Poedit). This is
> especially important when using
> [`cloud-translate`](https://github.com/mgeisler/cloud-translate) for initial
> translation as all entries will be marked as "fuzzy".
>
> If your text isn't translated, double-check that you have removed all "fuzzy"
> flags from your `xx.po` file.

### Building a Translated Book

The translation is done using the `mdbook-gettext` preprocessor. Enable it in
your project by adding this snippet to your `book.toml` file:

```toml
[preprocessor.gettext]
after = ["links"]
```

This will run `mdbook-gettext` on the source after things like `{{ #include }}`
has been executed. This makes it possible to translate included source code.

You can leave `mdbook-gettext` enabled: if no language is set or if it cannot
find the `.po` file corresponding to the language (e.g., it cannot find
`po/en.po` for English), then it will return the book untranslated.

To use the `po/xx.po` file for your output, you simply set `book.language` to
`xx`. You can do this on the command line:

```shell
MDBOOK_BOOK__LANGUAGE=xx mdbook build -d book/xx
```

This will set the book's language to `xx` and store the generated files in
`book/xx`.

### Serving a Translated Book

Like normal, you can use `mdbook serve` to view your translation as you work on
it. You use the same command as with `mdbook build` above:

```shell
MDBOOK_BOOK__LANGUAGE=xx mdbook serve -d book/xx
```

To automatically reload the book when you change the `po/xx.po` file, add this
to your `book.toml` file:

```toml
[build]
extra-watch-dirs = ["po"]
```

### Publishing Translations with GitHub Actions

Please see the [`publish.yml`] workflow in the Comprehensive Rust 🦀 repository.

[`publish.yml`]: https://github.com/google/comprehensive-rust/blob/main/.github/workflows/publish.yml

## Marking Sections with a comment

A block can be marked with a comment for translation by prepending a special
HTML comment `<!-- i18n:comment: XXX -->` in front of it. Consecutive HTML
comments will be collected into a single translation comment.

For example:

```markdown
The following will have a comment attached to the message.

But what is a man,

<!-- i18n:comment: ...a miserable little pile of secrets. -->
<!-- i18n:comment: But enough talk! -->

what has he got. If not himself, then he has naught.
```

## Marking Sections to be Skipped for Translation

A block can be marked to be skipped for translation by prepending a special HTML
comment `<!-- i18n:skip -->` in front of it.

For example:

````markdown
The following code block should not be translated.

<!-- i18n:skip -->

```
fn hello() {
  println!("Hello world!");
}
```

Itemized list:

- A should be translated.

<!-- i18n:skip -->

- B should be skipped.
- C should be translated.
````

Note that we don't extract the full text of code blocks. Only text that is
recognized as comments and literal strings is extracted.

## Normalizing Existing PO Files

When mdbook-i18n-helpers change, the generated PO files change as well. This can
result in a situation where the messages in a `xx.po` file are no longer exactly
like the ones expected by `mdbook-gettext`.

An example is the change from version 0.1.0 to 0.2.0: `mdbook-xgettext` from
version 0.1.0 will output a list as a whole:

```markdown
- foo
- bar
```

becomes

```gettext
msgid ""
"- foo\n"
"- bar\n"
msgstr ""
```

in the PO file. However, `mdbook-xgettext` version 0.2.0 will produce two
messages instead:

```gettext
msgid "foo"
msgstr ""

msgid "bar"
msgstr ""
```

Use `mdbook-i18n-normalize` version 0.2.0 to convert the old PO file to the new
format. Importantly, existing translations are kept intact! If the old PO file
is translated like this

```gettext
msgid ""
"- foo\n"
"- bar\n"
msgstr ""
"- FOO\n"
"- BAR\n"
```

then the new PO file generated with `mdbook-i18n-normalize` will contain two
messages:

```gettext
msgid "foo"
msgstr "FOO"

msgid "bar"
msgstr "BAR"
```

You will only need to run `mdbook-i18n-normalize` once after upgrading
mdbook-i18n-helpers.
