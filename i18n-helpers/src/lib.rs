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

use polib::catalog::Catalog;
use polib::message::Message;
use pulldown_cmark::{Event, LinkType, Tag};
use pulldown_cmark_to_cmark::{cmark_resume_with_options, Options, State};
use regex::Regex;
use std::sync::OnceLock;

pub mod normalize;

/// Like `mdbook::utils::new_cmark_parser`, but also passes a
/// `BrokenLinkCallback`.
pub fn new_cmark_parser<'input, 'callback>(
    text: &'input str,
    broken_link_callback: pulldown_cmark::BrokenLinkCallback<'input, 'callback>,
) -> pulldown_cmark::Parser<'input, 'callback> {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    options.insert(pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES);
    pulldown_cmark::Parser::new_with_broken_link_callback(text, options, broken_link_callback)
}

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

    fn expand_shortcut_link(tag: Tag) -> Tag {
        match tag {
            Tag::Link(LinkType::Shortcut, reference, title) => {
                Tag::Link(LinkType::Reference, reference, title)
            }
            Tag::Image(LinkType::Shortcut, reference, title) => {
                Tag::Image(LinkType::Reference, reference, title)
            }
            _ => tag,
        }
    }

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
        _ => new_cmark_parser(text, None)
            .into_offset_iter()
            .map(|(event, range)| {
                let lineno = offsets.partition_point(|&o| o < range.start) + 1;
                let event = match event {
                    Event::SoftBreak => Event::Text(" ".into()),
                    // Shortcut links like "[foo]" end up as "[foo]"
                    // in output. By changing them to a reference
                    // link, the link is expanded on the fly and the
                    // output becomes self-contained.
                    Event::Start(tag @ Tag::Link(..) | tag @ Tag::Image(..)) => {
                        Event::Start(expand_shortcut_link(tag))
                    }
                    Event::End(tag @ Tag::Link(..) | tag @ Tag::Image(..)) => {
                        Event::End(expand_shortcut_link(tag))
                    }
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
/// let events = extract_events("- A list item.", None);
/// assert_eq!(
///     events,
///     vec![
///         (1, Event::Start(Tag::List(None))),
///         (1, Event::Start(Tag::Item)),
///         (1, Event::Text("A list item.".into())),
///         (1, Event::End(Tag::Item)),
///         (1, Event::End(Tag::List(None))),
///     ],
/// );
///
/// let groups = group_events(&events);
/// assert_eq!(
///     groups,
///     vec![
///         Group::Skip(&[
///             (1, Event::Start(Tag::List(None))),
///             (1, Event::Start(Tag::Item)),
///         ]),
///         Group::Translate(&[
///             (1, Event::Text("A list item.".into())),
///         ]),
///         Group::Skip(&[
///             (1, Event::End(Tag::Item)),
///             (1, Event::End(Tag::List(None))),
///         ]),
///     ]
/// );
/// ```
pub fn group_events<'a>(events: &'a [(usize, Event<'a>)]) -> Vec<Group<'a>> {
    let mut groups = Vec::new();

    #[derive(Debug)]
    struct GroupingContext {
        skip_next_group: bool,
        // TODO: this struct is planned to expand with translator
        // comments and message contexts.
    }
    impl GroupingContext {
        fn clear_skip_next_group(self) -> Self {
            Self {
                skip_next_group: false,
            }
        }
    }

    #[derive(Debug)]
    enum State {
        Translate(usize),
        Skip(usize),
    }

    impl State {
        /// Creates a group based on the capturing state and context.
        fn into_group<'a>(
            self,
            idx: usize,
            events: &'a [(usize, Event<'a>)],
            ctx: GroupingContext,
        ) -> (Group<'a>, GroupingContext) {
            match self {
                State::Translate(start) => {
                    if ctx.skip_next_group {
                        (
                            Group::Skip(&events[start..idx]),
                            ctx.clear_skip_next_group(),
                        )
                    } else if is_nontranslatable_codeblock_group(&events[start..idx]) {
                        (Group::Skip(&events[start..idx]), ctx)
                    } else {
                        (Group::Translate(&events[start..idx]), ctx)
                    }
                }
                State::Skip(start) => (Group::Skip(&events[start..idx]), ctx),
            }
        }
    }

    let mut state = State::Skip(0);
    let mut ctx = GroupingContext {
        skip_next_group: false,
    };

    for (idx, (_, event)) in events.iter().enumerate() {
        match event {
            // These block-level events force new groups. We do this
            // because we want to include these events in the group to
            // make the group self-contained.
            Event::Start(Tag::Paragraph | Tag::CodeBlock(..)) => {
                // A translatable group starts here.
                let next_group;
                (next_group, ctx) = state.into_group(idx, events, ctx);
                groups.push(next_group);

                state = State::Translate(idx);
            }
            Event::End(Tag::Paragraph | Tag::CodeBlock(..)) => {
                // A translatable group ends after `idx`.
                let idx = idx + 1;
                let next_group;
                (next_group, ctx) = state.into_group(idx, events, ctx);
                groups.push(next_group);

                state = State::Skip(idx);
            }

            // Inline events start or continue a translating group.
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
                if let State::Skip(_) = state {
                    let next_group;
                    (next_group, ctx) = state.into_group(idx, events, ctx);
                    groups.push(next_group);

                    state = State::Translate(idx);
                }
            }

            // An HTML comment directive to skip the next translation
            // group.
            Event::Html(s) if is_comment_skip_directive(s) => {
                // If in the middle of translation, finish it.
                if let State::Translate(_) = state {
                    let next_group;
                    (next_group, ctx) = state.into_group(idx, events, ctx);
                    groups.push(next_group);

                    // Restart translation: subtle but should be
                    // needed to handle the skipping of the rest of
                    // the inlined content.
                    state = State::Translate(idx);
                }

                ctx.skip_next_group = true;
            }

            // All other block-level events start or continue a
            // skipping group.
            _ => {
                if let State::Translate(_) = state {
                    let next_group;
                    (next_group, ctx) = state.into_group(idx, events, ctx);
                    groups.push(next_group);

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

/// Check whether the HTML is a directive to skip the next translation group.
fn is_comment_skip_directive(html: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();

    let re =
        RE.get_or_init(|| Regex::new(r"<!-{2,}\s*mdbook-xgettext\s*:\s*skip\s*-{2,}>").unwrap());
    re.is_match(html.trim())
}

/// Returns true if the events appear to be a codeblock without translatable text.
fn is_nontranslatable_codeblock_group(events: &[(usize, Event)]) -> bool {
    match events {
        [(_, Event::Start(Tag::CodeBlock(_))), .., (_, Event::End(Tag::CodeBlock(_)))] => {
            let (codeblock_text, _) = reconstruct_markdown(events, None);
            // Heuristic to check whether the codeblock nether has a
            // literal string nor a line comment.  We may actually
            // want to use a lexer here to make this more robust.
            !codeblock_text.contains('"') && !codeblock_text.contains("//")
        }
        _ => false,
    }
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

    // Block quotes and lists add padding to the state, which is
    // reflected in the rendered Markdown. We want to capture the
    // Markdown without the padding to remove the effect of these
    // structural elements. Similarly, we don't want extra newlines at
    // the start.
    let simplified_state = state.map(|state| State {
        newlines_before_start: 0,
        padding: Vec::new(),
        ..state
    });
    cmark_resume_with_options(events, &mut markdown, simplified_state, options).unwrap();
    // Even with `newlines_before_start` set to zero, we get a leading
    // `\n` for code blocks (since they must start on a new line). We
    // can safely trim this here since we know that we always
    // reconstruct Markdown for a self-contained group of events.
    (String::from(markdown.trim_matches('\n')), new_state)
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

/// Builds messages and populates Catalog with translatable text extracted from Markdown file
///
pub fn add_message(catalog: &mut Catalog, msgid: &str, source: &str) {
    let sources = match catalog.find_message(None, msgid, None) {
        Some(msg) => format!("{}\n{}", msg.source(), source),
        None => String::from(source),
    };
    let message = Message::build_singular()
        .with_source(sources)
        .with_msgid(String::from(msgid))
        .done();
    catalog.append_or_update(message);
}

/// Trim `new_events` if they're wrapped in an unwanted paragraph.
///
/// If `new_events` is wrapped in a paragraph and `old_events` isn't,
/// then the paragraph is removed. This is useful when a text event
/// has been wrapped in a paragraph:
///
/// ```
/// use pulldown_cmark::{Event, Tag};
/// use mdbook_i18n_helpers::{extract_events, reconstruct_markdown, trim_paragraph};
///
/// let old_events = vec![(1, Event::Text("A line of text".into()))];
/// let (markdown, _) = reconstruct_markdown(&old_events, None);
/// let new_events = extract_events(&markdown, None);
/// // The stand-alone text has been wrapped in an extra paragraph:
/// assert_eq!(
///     new_events,
///     &[
///         (1, Event::Start(Tag::Paragraph)),
///         (1, Event::Text("A line of text".into())),
///         (1, Event::End(Tag::Paragraph)),
///     ],
/// );
///
/// assert_eq!(
///     trim_paragraph(&new_events, &old_events),
///     &[(1, Event::Text("A line of text".into()))],
/// );
/// ```
pub fn trim_paragraph<'a, 'event>(
    new_events: &'a [(usize, Event<'event>)],
    old_events: &'a [(usize, Event<'event>)],
) -> &'a [(usize, Event<'event>)] {
    use pulldown_cmark::Event::{End, Start};
    use pulldown_cmark::Tag::Paragraph;
    match new_events {
        [(_, Start(Paragraph)), inner @ .., (_, End(Paragraph))] => match old_events {
            [(_, Start(Paragraph)), .., (_, End(Paragraph))] => new_events,
            [..] => inner,
        },
        [..] => new_events,
    }
}

/// Translate `events` using `catalog`.
pub fn translate_events<'a>(
    events: &'a [(usize, Event<'a>)],
    catalog: &'a Catalog,
) -> Vec<(usize, Event<'a>)> {
    let mut translated_events = Vec::new();
    let mut state = None;

    for group in group_events(events) {
        match group {
            Group::Translate(events) => {
                // Reconstruct the message.
                let (msgid, new_state) = reconstruct_markdown(events, state.clone());
                let translated = catalog
                    .find_message(None, &msgid, None)
                    .filter(|msg| !msg.flags().is_fuzzy())
                    .and_then(|msg| msg.msgstr().ok())
                    .filter(|msgstr| !msgstr.is_empty());
                match translated {
                    Some(msgstr) => {
                        // Generate new events for `msgstr`, taking
                        // care to trim away unwanted paragraphs.
                        translated_events.extend_from_slice(trim_paragraph(
                            &extract_events(msgstr, state),
                            events,
                        ));
                    }
                    None => translated_events.extend_from_slice(events),
                }
                // Advance the state.
                state = Some(new_state);
            }
            Group::Skip(events) => {
                // Copy the events unchanged to the output.
                translated_events.extend_from_slice(events);
                // Advance the state.
                let (_, new_state) = reconstruct_markdown(events, state);
                state = Some(new_state);
            }
        }
    }

    translated_events
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use pulldown_cmark::CodeBlockKind;
    use pulldown_cmark::Event::*;
    use pulldown_cmark::HeadingLevel::*;
    use pulldown_cmark::Tag::*;

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
    fn extract_events_empty() {
        assert_eq!(extract_events("", None), vec![]);
    }

    #[test]
    fn extract_events_paragraph() {
        assert_eq!(
            extract_events("foo bar", None),
            vec![
                (1, Start(Paragraph)),
                (1, Text("foo bar".into())),
                (1, End(Paragraph)),
            ]
        );
    }

    #[test]
    fn extract_events_softbreak() {
        assert_eq!(
            extract_events("foo\nbar", None),
            vec![
                (1, Start(Paragraph)),
                (1, Text("foo".into())),
                (1, Text(" ".into())),
                (2, Text("bar".into())),
                (1, End(Paragraph)),
            ]
        );
    }

    #[test]
    fn extract_events_heading() {
        assert_eq!(
            extract_events("# Foo Bar", None),
            vec![
                (1, Start(Heading(H1, None, vec![]))),
                (1, Text("Foo Bar".into())),
                (1, End(Heading(H1, None, vec![]))),
            ]
        );
    }

    #[test]
    fn extract_events_list_item() {
        assert_eq!(
            extract_events("* foo bar", None),
            vec![
                (1, Start(List(None))),
                (1, Start(Item)),
                (1, Text("foo bar".into())),
                (1, End(Item)),
                (1, End(List(None))),
            ]
        );
    }

    #[test]
    fn extract_events_code_block() {
        let (_, state) =
            reconstruct_markdown(&[(1, Start(CodeBlock(CodeBlockKind::Indented)))], None);
        assert_eq!(
            extract_events("foo\nbar\nbaz", Some(state)),
            vec![
                (1, Text("foo\n".into())),
                (2, Text("bar\n".into())),
                (3, Text("baz".into())),
            ]
        );

        // Compare with extraction without state:
        assert_eq!(
            extract_events("foo\nbar\nbaz", None),
            vec![
                (1, Start(Paragraph)),
                (1, Text("foo".into())),
                (1, Text(" ".into())),
                (2, Text("bar".into())),
                (2, Text(" ".into())),
                (3, Text("baz".into())),
                (1, End(Paragraph)),
            ]
        );
    }

    #[test]
    fn extract_events_comments() {
        assert_eq!(
            extract_events("<!-- mdbook-xgettext:skip -->\nHello", None),
            vec![
                (1, Html("<!-- mdbook-xgettext:skip -->\n".into())),
                (2, Start(Paragraph)),
                (2, Text("Hello".into())),
                (2, End(Paragraph)),
            ]
        );
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
    fn extract_messages_inline_link() {
        assert_extract_messages(
            "See [this page](https://example.com) for more info.",
            vec![(1, "See [this page](https://example.com) for more info.")],
        );
    }

    #[test]
    fn extract_messages_reference_link() {
        assert_extract_messages(
            "See [this page][1] for more info.\n\n\
             [1]: https://example.com",
            // The parser expands reference links on the fly.
            vec![(1, "See [this page](https://example.com) for more info.")],
        );
    }

    #[test]
    fn extract_messages_collapsed_link() {
        // We make the parser expand collapsed links on the fly.
        assert_extract_messages(
            "Click [here][]!\n\n\
             [here]: http://example.net/",
            vec![(1, "Click [here](http://example.net/)!")],
        );
    }

    #[test]
    fn extract_messages_shortcut_link() {
        assert_extract_messages(
            "Click [here]!\n\n\
             [here]: http://example.net/",
            vec![(1, "Click [here](http://example.net/)!")],
        );
    }

    #[test]
    fn extract_messages_autolink() {
        assert_extract_messages(
            "Visit <http://example.net>!",
            vec![(1, "Visit <http://example.net>!")],
        );
    }

    #[test]
    fn extract_messages_email() {
        assert_extract_messages(
            "Contact <info@example.net>!",
            vec![(1, "Contact <info@example.net>!")],
        );
    }

    #[test]
    fn extract_messages_broken_reference_link() {
        // A reference link without the corresponding link definition
        // results in an escaped link.
        //
        // See `SourceMap::extract_messages` for a more complex
        // approach which can work around this in some cases.
        assert_extract_messages("[foo][unknown]", vec![(1, r"\[foo\]\[unknown\]")]);
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
            input,
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
            "Preamble\n```rust\n// Example:\nfn hello() {\n  some_code()\n\n  todo!()\n}\n```\nPostamble",
            vec![
                (1, "Preamble"),
                (
                    2,
                    "```rust\n// Example:\nfn hello() {\n  some_code()\n\n  todo!()\n}\n```",
                ),
                (10, "Postamble"),
            ],
        );
    }

    #[test]
    fn extract_messages_two_code_blocks() {
        assert_extract_messages(
            "```\n\
             \"First\" block\n\
             ```\n\
             ```\n\
             \"Second\" block\n\
             ```\n\
             ",
            vec![
                (1, "```\n\"First\" block\n```"), //
                (4, "```\n\"Second\" block\n```"),
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
            >     // FIXME: do something here!\n\
            >     todo!()\n\
            > }\n\
            > ```\n\
            > Postamble",
            vec![
                (1, "Preamble"),
                (
                    2,
                    "```rust\nfn hello() {\n    some_code()\n\n    // FIXME: do something here!\n    todo!()\n}\n```",
                ),
                (10, "Postamble"),
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
// BOB
```

<details>

* Blah blah

</details>
"#,
            vec![
                (1, "```bob\n// BOB\n```"), //
                (7, "Blah blah"),
            ],
        );
    }

    #[test]
    fn test_is_comment_skip_directive_simple() {
        assert_eq!(
            is_comment_skip_directive("<!-- mdbook-xgettext:skip -->"),
            true
        );
    }

    #[test]
    fn test_is_comment_skip_directive_tolerates_spaces() {
        assert_eq!(
            is_comment_skip_directive("<!-- mdbook-xgettext: skip -->"),
            true
        );
    }

    #[test]
    fn test_is_comment_skip_directive_tolerates_dashes() {
        assert_eq!(
            is_comment_skip_directive("<!--- mdbook-xgettext:skip ---->"),
            true
        );
    }

    #[test]
    fn test_is_comment_skip_directive_needs_skip() {
        assert_eq!(
            is_comment_skip_directive("<!-- mdbook-xgettext: foo -->"),
            false
        );
    }
    #[test]
    fn test_is_comment_skip_directive_needs_to_be_a_comment() {
        assert_eq!(
            is_comment_skip_directive("<div>mdbook-xgettext: skip</div>"),
            false
        );
    }

    #[test]
    fn extract_messages_skip_simple() {
        assert_extract_messages(
            r#"<!-- mdbook-xgettext:skip -->

This is a paragraph."#,
            vec![],
        );
    }

    #[test]
    fn extract_messages_skip_next_paragraph_ok() {
        assert_extract_messages(
            r#"<!-- mdbook-xgettext:skip -->
This is a paragraph.

This should be translated.
"#,
            vec![(4, "This should be translated.")],
        );
    }

    #[test]
    fn extract_messages_skip_next_codeblock() {
        assert_extract_messages(
            r#"<!-- mdbook-xgettext:skip -->
```
def f(x): return x * x
```
This should be translated.
"#,
            vec![(5, "This should be translated.")],
        );
    }

    #[test]
    fn extract_messages_skip_back_to_back() {
        assert_extract_messages(
            r#"<!-- mdbook-xgettext:skip -->
```
def f(x): return x * x
```
<!-- mdbook-xgettext:skip -->
This should not translated.

But *this* should!
"#,
            vec![(8, "But _this_ should!")],
        );
    }

    #[test]
    fn extract_messages_inline_skips() {
        assert_extract_messages(
            "
this should be translated <!-- mdbook-xgettext:skip --> but not this.
... nor this.

But *this* should!",
            vec![(2, "this should be translated "), (5, "But _this_ should!")],
        );
    }

    #[test]
    fn extract_messages_skipping_second_item() {
        assert_extract_messages(
            "
* A
<!-- mdbook-xgettext:skip -->
* B
* C
",
            vec![(2, "A"), (5, "C")],
        );
    }

    #[test]
    fn extract_messages_skipping_second_paragraphed_item() {
        assert_extract_messages(
            "
* A

<!-- mdbook-xgettext:skip -->
* B

* C
",
            vec![(2, "A"), (7, "C")],
        );
    }

    #[test]
    fn extract_messages_skipping_inline_second_item() {
        // This isn't great: we lose text following a HTML comment.
        // Very similar to the failure mode of the
        // `extract_messages_details` test.
        //
        // The root cause is due to the Markdown spec and how the
        // Markdown parser treats HTML blocks.  The text that
        // immediately follows an HTML block on the same line is
        // included as part of the HTML block.
        assert_extract_messages(
            "
* A
* <!-- mdbook-xgettext:skip --> B
* C
",
            vec![(2, "A")],
        );
    }

    #[test]
    fn extract_messages_inline_skip_to_end_of_block() {
        assert_extract_messages(
            "foo <!-- mdbook-xgettext:skip --> **bold** bar
still skipped

not-skipped",
            vec![(1, "foo "), (4, "not-skipped")],
        );
    }

    #[test]
    fn extract_messages_automatic_skipping_nontranslatable_codeblocks_simple() {
        assert_extract_messages(
            r#"
```
def g(x):
  this_should_be_skipped_no_strings_or_comments()
```
"#,
            vec![],
        );
    }

    #[test]
    fn extract_messages_automatic_skipping_nontranslatable_codeblocks() {
        assert_extract_messages(
            r#"
```
def f(x):
  print("this should be translated")
```


```
def g(x):
  but_this_should_not()
```
"#,
            vec![(
                2,
                "```\ndef f(x):\n  print(\"this should be translated\")\n```",
            )],
        );
    }

    #[test]
    fn is_nontranslatable_codeblock_group_true() {
        let events = extract_events(
            r#"```
f(x)
```"#,
            None,
        );
        assert!(is_nontranslatable_codeblock_group(&events));
    }

    #[test]
    fn is_nontranslatable_codeblock_group_false() {
        let events = extract_events(
            r#"```
f("hello world")
```"#,
            None,
        );
        assert!(!is_nontranslatable_codeblock_group(&events));
    }
}
