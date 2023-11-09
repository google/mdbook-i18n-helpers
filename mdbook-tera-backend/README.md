# Tera backend extension for `mdbook`

[![Visit crates.io](https://img.shields.io/crates/v/mdbook-i18n-helpers?style=flat-square)](https://crates.io/crates/mdbook-tera-backend)
[![Build workflow](https://img.shields.io/github/actions/workflow/status/google/mdbook-i18n-helpers/test.yml?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/actions/workflows/test.yml?query=branch%3Amain)
[![GitHub contributors](https://img.shields.io/github/contributors/google/mdbook-i18n-helpers?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/graphs/contributors)
[![GitHub stars](https://img.shields.io/github/stars/google/mdbook-i18n-helpers?style=flat-square)](https://github.com/google/mdbook-i18n-helpers/stargazers)

This `mdbook` backend makes it possible to use
[tera](https://github.com/Keats/tera) templates and expand the capabilities of
your books. It works on top of the default HTML backend.

## Installation

Run

```shell
$ cargo install mdbook-tera-backend
```

## Usage

### Configuring the backend

To enable the backend, simply add `[output.tera-backend]` to your `book.toml`,
and configure the place where youre templates will live. For instance
`theme/templates`:

```toml
[output.html] # You must still enable the html backend.
[output.tera-backend]
template_dir = "theme/templates"
```

### Creating templates

Create your template files in the same directory as your book.

```html
<!-- ./theme/templates/hello_world.html -->
<div>
  Hello world!
</div>
```

### Using templates in `index.hbs`

Since the HTML renderer will first render Handlebars templates, we need to tell
it to ignore Tera templates using `{{{{raw}}}}` blocks:

```html
{{{{raw}}}}
{% set current_language = ctx.config.book.language %}
<p>Current language: {{ current_language }}</p>
{% include "hello_world.html" %}
{{{{/raw}}}}
```

Includes names are based on the file name and not the whole file path.

### Tera documentation

Find out all you can do with Tera templates
[here](https://keats.github.io/tera/docs/).

## Changelog

Please see [CHANGELOG](../CHANGELOG.md) for details on the changes in each
release.

## Contact

For questions or comments, please contact
[Martin Geisler](mailto:mgeisler@google.com) or
[Alexandre Senges](mailto:asenges@google.come) or start a
[discussion](https://github.com/google/mdbook-i18n-helpers/discussions). We
would love to hear from you.

---

This is not an officially supported Google product.
