// Copyright 2023 Google LLC
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

//! Helpers for translating `mdbook` projects.
//!
//! The functions here are used to implement a robust
//! internationalization (i18n) workflow for `mdbook`. This allows you
//! to translate your books into other languages while also making it
//! easy to keep the translations up to date as you edit the original
//! source text.
//!
//! See <https://github.com/google/mdbook-i18n-helpers> for details on
//! how to use the supplied `mdbook` plugins.

use mdbook::utils::new_cmark_parser;
use pulldown_cmark::{Event, Tag};
use pulldown_cmark_to_cmark::{cmark_resume_with_options, Options, State};

/// Extract Markdown events from `text`.
///
/// The `state` can be used to give the parsing context. In
/// particular, if a code block has started, the text should be parsed
/// without interpreting special Markdown characters.
///
/// The events are labeled with the line number where they start in
/// the document.
///
/// # Examples
///
/// ```
/// use mdbook_i18n_helpers::extract_events;
/// use pulldown_cmark::{Event, Tag};
///
/// assert_eq!(
///     extract_events("Hello,\nworld!", None),
///     vec![
///         (1, Event::Start(Tag::Paragraph)),
///         (1, Event::Text("Hello,".into())),
///         (1, Event::Text(" ".into())),
///         (2, Event::Text("world!".into())),
///         (1, Event::End(Tag::Paragraph)),
///     ]
/// );
/// ```
pub fn extract_events<'a>(text: &'a str, state: Option<State<'static>>) -> Vec<(usize, Event<'a>)> {
    // Offsets of each newline in the input, used to calculate line
    // numbers from byte offsets.
    let offsets = text
        .match_indices('\n')
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();

    match state {
        // If we're in a code block, we disable the normal parsing and
        // return lines of text. This matches the behavior of the
        // parser in this case.
        Some(state) if state.is_in_code_block => text
            .split_inclusive('\n')
            .enumerate()
            .map(|(idx, line)| (idx + 1, Event::Text(line.into())))
            .collect(),
        // Otherwise, we parse the text line normally.
        _ => new_cmark_parser(text, false)
            .into_offset_iter()
            .map(|(event, range)| {
                let lineno = offsets.partition_point(|&o| o < range.start) + 1;
                let event = match event {
                    Event::SoftBreak => Event::Text(" ".into()),
                    _ => event,
                };
                (lineno, event)
            })
            .collect(),
    }
}

/// Markdown events grouped by type.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Group<'a> {
    /// Markdown events which should be translated.
    ///
    /// This includes `[Text("foo")]` as well as sequences with text
    /// such as `[Start(Emphasis), Text("foo") End(Emphasis)]`.
    Translate(&'a [(usize, Event<'a>)]),

    /// Markdown events which should be skipped when translating.
    ///
    /// This includes structural events such as `Start(Heading(H1,
    /// None, vec![]))`.
    Skip(&'a [(usize, Event<'a>)]),
}

/// Group Markdown events into translatable and skipped events.
///
/// This function will partition the input events into groups of
/// events which should be translated or skipped. Concatenating the
/// events in each group will give you back the original events.
///
/// # Examples
///
/// ```
/// use mdbook_i18n_helpers::{extract_events, group_events, Group};
/// use pulldown_cmark::{Event, Tag};
///
/// let events = extract_events("This is a _paragraph_ of text.", None);
/// assert_eq!(
///     events,
///     vec![
///         (1, Event::Start(Tag::Paragraph)),
///         (1, Event::Text("This is a ".into())),
///         (1, Event::Start(Tag::Emphasis)),
///         (1, Event::Text("paragraph".into())),
///         (1, Event::End(Tag::Emphasis)),
///         (1, Event::Text(" of text.".into())),
///         (1, Event::End(Tag::Paragraph)),
///     ],
/// );
///
/// let groups = group_events(&events);
/// assert_eq!(
///     groups,
///     vec![
///         Group::Skip(&[
///             (1, Event::Start(Tag::Paragraph)),
///         ]),
///         Group::Translate(&[
///             (1, Event::Text("This is a ".into())),
///             (1, Event::Start(Tag::Emphasis)),
///             (1, Event::Text("paragraph".into())),
///             (1, Event::End(Tag::Emphasis)),
///             (1, Event::Text(" of text.".into())),
///         ]),
///         Group::Skip(&[
///             (1, Event::End(Tag::Paragraph)),
///         ]),
///     ]
/// );
/// ```
pub fn group_events<'a>(events: &'a [(usize, Event<'a>)]) -> Vec<Group<'a>> {
    let mut groups = Vec::new();

    enum State {
        Translate(usize),
        Skip(usize),
    }
    let mut state = State::Skip(0);

    for (idx, (_, event)) in events.iter().enumerate() {
        match event {
            Event::Start(
                Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link(..) | Tag::Image(..),
            )
            | Event::End(
                Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link(..) | Tag::Image(..),
            )
            | Event::Text(_)
            | Event::Code(_)
            | Event::FootnoteReference(_)
            | Event::SoftBreak
            | Event::HardBreak => {
                // If we're currently skipping, then a new
                // translatable group starts here.
                if let State::Skip(start) = state {
                    groups.push(Group::Skip(&events[start..idx]));
                    state = State::Translate(idx);
                }
            }
            _ => {
                // If we're currently translating, then a new
                // skippable group starts here.
                if let State::Translate(start) = state {
                    groups.push(Group::Translate(&events[start..idx]));
                    state = State::Skip(idx);
                }
            }
        }
    }

    match state {
        State::Translate(start) => groups.push(Group::Translate(&events[start..])),
        State::Skip(start) => groups.push(Group::Skip(&events[start..])),
    }

    groups
}

/// Render a slice of Markdown events back to Markdown.
///
/// # Examples
///
/// ```
/// use mdbook_i18n_helpers::{extract_events, reconstruct_markdown};
/// use pulldown_cmark::{Event, Tag};
///
/// let group = extract_events("Hello *world!*", None);
/// let (reconstructed, _) = reconstruct_markdown(&group, None);
/// assert_eq!(reconstructed, "Hello _world!_");
/// ```
///
/// Notice how this will normalize the Markdown to use `_` for
/// emphasis and `**` for strong emphasis. The style is chosen to
/// match the [Google developer documentation style
/// guide](https://developers.google.com/style/text-formatting).
pub fn reconstruct_markdown(
    group: &[(usize, Event)],
    state: Option<State<'static>>,
) -> (String, State<'static>) {
    let events = group.iter().map(|(_, event)| event);
    let mut markdown = String::new();
    let options = Options {
        code_block_token_count: 3,
        list_token: '-',
        emphasis_token: '_',
        strong_token: "**",
        ..Options::default()
    };
    // Advance the true state, but throw away the rendered Markdown
    // since it can contain unwanted padding.
    let new_state = cmark_resume_with_options(
        events.clone(),
        String::new(),
        state.clone(),
        options.clone(),
    )
    .unwrap();

    // Block quotes and lists add padding to the state. This is
    // reflected in the rendered Markdown. We want to capture the
    // Markdown without the padding to remove the effect of these
    // structural elements.
    let state_without_padding = state.map(|state| State {
        padding: Vec::new(),
        ..state
    });
    cmark_resume_with_options(events, &mut markdown, state_without_padding, options).unwrap();
    (markdown, new_state)
}

/// Extract translatable strings from `document`.
///
/// # Examples
///
/// Structural markup like headings and lists are removed from the
/// messages:
///
/// ```
/// use mdbook_i18n_helpers::extract_messages;
///
/// assert_eq!(
///     extract_messages("# A heading"),
///     vec![(1, "A heading".into())],
/// );
/// assert_eq!(
///     extract_messages(
///         "1. First item\n\
///          2. Second item\n"
///     ),
///     vec![
///         (1, "First item".into()),
///         (2, "Second item".into()),
///     ],
/// );
/// ```
///
/// Indentation due to structural elements like block quotes and lists
/// is ignored:
///
/// ```
/// use mdbook_i18n_helpers::extract_messages;
///
/// let messages = extract_messages(
///     "> *   Hello, this is a\n\
///      >     list in a quote.\n\
///      >\n\
///      >     This is the second\n\
///      >     paragraph.\n"
/// );
/// assert_eq!(
///     messages,
///     vec![
///         (1, "Hello, this is a list in a quote.".into()),
///         (4, "This is the second paragraph.".into()),
///     ],
/// );
/// ```
pub fn extract_messages(document: &str) -> Vec<(usize, String)> {
    let events = extract_events(document, None);
    let mut messages = Vec::new();
    let mut state = None;
    for group in group_events(&events) {
        match group {
            Group::Translate(events) => {
                if let Some((lineno, _)) = events.first() {
                    let (text, new_state) = reconstruct_markdown(events, state);
                    messages.push((*lineno, text));
                    state = Some(new_state);
                }
            }
            Group::Skip(events) => {
                let (_, new_state) = reconstruct_markdown(events, state);
                state = Some(new_state);
            }
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Extract messages in `document`, assert they match `expected`.
    #[track_caller]
    fn assert_extract_messages(document: &str, expected: Vec<(usize, &str)>) {
        assert_eq!(
            extract_messages(document)
                .iter()
                .map(|(lineno, msg)| (*lineno, &msg[..]))
                .collect::<Vec<_>>(),
            expected,
        )
    }

    #[test]
    fn extract_messages_empty() {
        assert_extract_messages("", vec![]);
    }

    #[test]
    fn extract_messages_single_line() {
        assert_extract_messages("This is a paragraph.", vec![(1, "This is a paragraph.")]);
    }

    #[test]
    fn extract_messages_simple() {
        assert_extract_messages(
            "This is\n\
             the first\n\
             paragraph.🦀\n\
             \n\
             Second paragraph.",
            vec![
                (1, "This is the first paragraph.🦀"),
                (5, "Second paragraph."),
            ],
        );
    }

    #[test]
    fn extract_messages_leading_newlines() {
        assert_extract_messages(
            "\n\
             \n\
             \n\
             This is the\n\
             first paragraph.",
            vec![(4, "This is the first paragraph.")],
        );
    }

    #[test]
    fn extract_messages_trailing_newlines() {
        assert_extract_messages(
            "This is\n\
             a paragraph.\n\
             \n\
             \n",
            vec![(1, "This is a paragraph.")],
        );
    }

    #[test]
    fn extract_messages_styled_text() {
        // The parser normalizes "*emphasis*" to "_emphasis_" and
        // "__strong emphasis__" to "**strong emphasis**".
        assert_extract_messages(
            "**This** __~~message~~__ _has_ `code` *style*\n",
            vec![(1, "**This** **~~message~~** _has_ `code` _style_")],
        );
    }

    #[test]
    fn extract_messages_inline_html() {
        // HTML tags are skipped, but text inside is extracted:
        assert_extract_messages(
            "Hi <script>alert('there');</script>",
            vec![
                (1, "Hi "), //
                (1, "alert('there');"),
            ],
        );
    }

    #[test]
    fn extract_messages_links() {
        assert_extract_messages(
            "See [this page](https://example.com) for more info.",
            vec![(1, "See [this page](https://example.com) for more info.")],
        );
    }

    #[test]
    fn extract_messages_reference_links() {
        assert_extract_messages(
            r#"
* [Brazilian Portuguese][pt-BR] and
* [Korean][ko]

[pt-BR]: https://google.github.io/comprehensive-rust/pt-BR/
[ko]: https://google.github.io/comprehensive-rust/ko/
"#,
            // The parser expands reference links on the fly.
            vec![
                (2, "[Brazilian Portuguese](https://google.github.io/comprehensive-rust/pt-BR/) and"),
                (3, "[Korean](https://google.github.io/comprehensive-rust/ko/)"),
            ]
        );
    }

    #[test]
    fn extract_messages_footnotes() {
        assert_extract_messages(
            "
The document[^1] text.

[^1]: The footnote text.
",
            vec![
                (2, "The document[^1] text."), //
                (4, "The footnote text."),
            ],
        );
    }

    #[test]
    fn extract_messages_block_quote() {
        assert_extract_messages(
            r#"One of my favorite quotes is:

> Don't believe everything you read on the Internet.
>
> I didn't say this second part, but I needed a paragraph for testing.

--Abraham Lincoln
"#,
            vec![
                (1, "One of my favorite quotes is:"),
                (3, "Don't believe everything you read on the Internet."),
                (
                    5,
                    "I didn't say this second part, but I needed a paragraph for testing.",
                ),
                (7, "\\--Abraham Lincoln"),
            ],
        );
    }

    #[test]
    fn extract_messages_table() {
        let input = "\
            | Module Type       | Description\n\
            |-------------------|-------------------------\n\
            | `rust_binary`     | Produces a Rust binary.\n\
            | `rust_library`    | Produces a Rust library.\n\
        ";
        assert_extract_messages(
            &input,
            vec![
                (1, "Module Type"),
                (1, "Description"),
                (3, "`rust_binary`"),
                (3, "Produces a Rust binary."),
                (4, "`rust_library`"),
                (4, "Produces a Rust library."),
            ],
        );
    }

    #[test]
    fn extract_messages_code_block() {
        assert_extract_messages(
            "Preamble\n```rust\nfn hello() {\n  some_code()\n\n  todo!()\n}\n```\nPostamble",
            vec![
                (1, "Preamble"),
                (3, "fn hello() {\n  some_code()\n\n  todo!()\n}\n"),
                (9, "Postamble"),
            ],
        );
    }

    #[test]
    fn extract_messages_quoted_code_block() {
        assert_extract_messages(
            "\
            > Preamble\n\
            > ```rust\n\
            > fn hello() {\n\
            >     some_code()\n\
            >\n\
            >     todo!()\n\
            > }\n\
            > ```\n\
            > Postamble",
            vec![
                (1, "Preamble"),
                (3, "fn hello() {\n    some_code()\n\n    todo!()\n}\n"),
                (9, "Postamble"),
            ],
        );
    }

    #[test]
    fn extract_messages_details() {
        // This isn't great: we lose text following a HTML tag:
        assert_extract_messages(
            "Preamble\n\
             <details>\n\
             Some Details\n\
             </details>\n\
             \n\
             Postamble",
            vec![
                (1, "Preamble"), //
                // Missing "Some Details"
                (6, "Postamble"),
            ],
        );
        // It works well enough when `<details>` has blank lines
        // before and after.
        assert_extract_messages(
            "Preamble\n\
             \n\
             <details>\n\
             \n\
             Some Details\n\
             \n\
             </details>\n\
             \n\
             Postamble",
            vec![
                (1, "Preamble"), //
                (5, "Some Details"),
                (9, "Postamble"),
            ],
        );
    }

    #[test]
    fn extract_messages_list() {
        assert_extract_messages(
            "Some text\n * List item 1🦀\n * List item 2\n\nMore text",
            vec![
                (1, "Some text"), //
                (2, "List item 1🦀"),
                (3, "List item 2"),
                (5, "More text"),
            ],
        );
    }

    #[test]
    fn extract_messages_multilevel_list() {
        assert_extract_messages(
            "Some text\n * List item 1\n * List item 2\n    * Sublist 1\n    * Sublist 2\n\nMore text",
            vec![
                (1, "Some text"), //
                (2, "List item 1"),
                (3, "List item 2"),
                (4, "Sublist 1"),
                (5, "Sublist 2"),
                (7, "More text"),
            ],
        );
    }

    #[test]
    fn extract_messages_list_with_paragraphs() {
        assert_extract_messages(
            r#"* Item 1.
* Item 2,
  two lines.

  * Sub 1.
  * Sub 2.
"#,
            vec![
                (1, "Item 1."),
                (2, "Item 2, two lines."),
                (5, "Sub 1."),
                (6, "Sub 2."),
            ],
        );
    }

    #[test]
    fn extract_messages_headings() {
        assert_extract_messages(
            r#"Some text
# Headline News🦀

* A
* List

## Subheading
"#,
            vec![
                (1, "Some text"),
                (2, "Headline News🦀"),
                (4, "A"),
                (5, "List"),
                (7, "Subheading"),
            ],
        );
    }

    #[test]
    fn extract_messages_code_followed_by_details() {
        // This is a regression test for an error that would
        // incorrectly combine CodeBlock and HTML.
        assert_extract_messages(
            r#"```bob
BOB
```

<details>

* Blah blah

</details>
"#,
            vec![
                (2, "BOB\n"), //
                (7, "Blah blah"),
            ],
        );
    }
}
