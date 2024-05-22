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
use pulldown_cmark::{
    BrokenLinkCallback, CodeBlockKind, DefaultBrokenLinkCallback, Event, LinkType, Tag, TagEnd,
};
use pulldown_cmark_to_cmark::{
    calculate_code_block_token_count, cmark_resume_with_options, Options, State,
};
use std::sync::OnceLock;
use syntect::easy::ScopeRangeIterator;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

pub mod directives;
pub mod gettext;
pub mod normalize;
pub mod preprocessors;
pub mod xgettext;

/// Re-wrap the sources field of a message.
///
/// This function tries to wrap the `file:lineno` pairs so they look
/// the same as what you get from `msgcat` or `msgmerge`.
pub fn wrap_sources(sources: &str) -> String {
    let options = textwrap::Options::new(76)
        .break_words(false)
        .word_splitter(textwrap::WordSplitter::NoHyphenation);
    textwrap::refill(sources, options)
}

/// Like `mdbook::utils::new_cmark_parser`, but also passes a
/// `BrokenLinkCallback`.
pub fn new_cmark_parser<'input, F: BrokenLinkCallback<'input>>(
    text: &'input str,
    broken_link_callback: Option<F>,
) -> pulldown_cmark::Parser<'input, F> {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_OLD_FOOTNOTES);
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
/// use pulldown_cmark::{Event, Tag, TagEnd};
///
/// assert_eq!(
///     extract_events("Hello,\nworld!", None),
///     vec![
///         (1, Event::Start(Tag::Paragraph)),
///         (1, Event::Text("Hello,".into())),
///         (1, Event::Text(" ".into())),
///         (2, Event::Text("world!".into())),
///         (1, Event::End(TagEnd::Paragraph)),
///     ]
/// );
/// ```
pub fn extract_events<'a>(text: &'a str, state: Option<State<'a>>) -> Vec<(usize, Event<'a>)> {
    // Expand a `[foo]` style link into `[foo][foo]`.
    fn expand_shortcut_link(tag: Tag<'_>) -> Tag<'_> {
        match tag {
            Tag::Link {
                link_type: LinkType::Shortcut,
                dest_url,
                title,
                id,
            } => Tag::Link {
                link_type: LinkType::Reference,
                dest_url,
                title,
                id,
            },
            Tag::Image {
                link_type: LinkType::Shortcut,
                dest_url,
                title,
                id,
            } => Tag::Image {
                link_type: LinkType::Reference,
                dest_url,
                title,
                id,
            },
            _ => tag,
        }
    }

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
        _ => new_cmark_parser::<'a, DefaultBrokenLinkCallback>(text, None)
            .into_offset_iter()
            .map(|(event, range)| {
                let lineno = offsets.partition_point(|&o| o < range.start) + 1;
                let event = match event {
                    Event::SoftBreak => Event::Text(" ".into()),
                    // Shortcut links like "[foo]" end up as "[foo]"
                    // in output. By changing them to a reference
                    // link, the link is expanded on the fly and the
                    // output becomes self-contained.
                    Event::Start(tag @ (Tag::Link { .. } | Tag::Image { .. })) => {
                        Event::Start(expand_shortcut_link(tag))
                    }
                    _ => event,
                };
                (lineno, event)
            })
            .collect(),
    }
}

/// Markdown events grouped by type.
#[derive(Debug, Clone, PartialEq)]
pub enum Group<'a> {
    /// Markdown events which should be translated.
    ///
    /// This includes `[Text("foo")]` as well as sequences with text
    /// such as `[Start(Emphasis), Text("foo") End(Emphasis)]`.
    Translate {
        events: Vec<(usize, Event<'a>)>,
        /// A comment that may be associated with the translation text.
        comment: String,
    },

    /// Markdown events which should be skipped when translating.
    ///
    /// This includes structural events such as `Start(Heading(H1,
    /// None, vec![]))`.
    Skip(Vec<(usize, Event<'a>)>),
}

#[derive(Debug, Default)]
struct GroupingContext {
    skip_next_group: bool,
    comments: Vec<String>,
}

impl GroupingContext {
    fn clear_skip_next_group(self) -> Self {
        Self {
            skip_next_group: false,
            ..self
        }
    }
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
/// use pulldown_cmark::{Event, Tag, TagEnd};
///
/// let events = extract_events("- A list item.", None);
/// assert_eq!(
///     events,
///     vec![
///         (1, Event::Start(Tag::List(None))),
///         (1, Event::Start(Tag::Item)),
///         (1, Event::Text("A list item.".into())),
///         (1, Event::End(TagEnd::Item)),
///         (1, Event::End(TagEnd::List(false))),
///     ],
/// );
///
/// let groups = group_events(&events);
/// assert_eq!(
///     groups,
///     vec![
///         Group::Skip(vec![
///             (1, Event::Start(Tag::List(None))),
///             (1, Event::Start(Tag::Item)),
///         ]),
///         Group::Translate {
///             events: vec![
///                 (1, Event::Text("A list item.".into())),
///             ], comment: "".into()},
///         Group::Skip(vec![
///             (1, Event::End(TagEnd::Item)),
///             (1, Event::End(TagEnd::List(false))),
///         ]),
///     ]
/// );
/// ```
pub fn group_events<'a>(events: &'a [(usize, Event<'a>)]) -> Vec<Group<'a>> {
    #[derive(Debug)]
    enum State {
        Translate(usize),
        Skip(usize),
    }

    impl State {
        /// Creates groups based on the capturing state and context.
        fn into_groups<'a>(
            self,
            idx: usize,
            events: &'a [(usize, Event<'a>)],
            mut ctx: GroupingContext,
        ) -> (Vec<Group<'a>>, GroupingContext) {
            match self {
                State::Translate(start) => {
                    if ctx.skip_next_group {
                        (
                            vec![Group::Skip(events[start..idx].into())],
                            ctx.clear_skip_next_group(),
                        )
                    } else if is_codeblock_group(&events[start..idx]) {
                        parse_codeblock(&events[start..idx], ctx)
                    } else {
                        (
                            vec![Group::Translate {
                                events: events[start..idx].into(),
                                comment: std::mem::take(&mut ctx.comments).join(" "),
                            }],
                            ctx,
                        )
                    }
                }
                State::Skip(start) => (vec![Group::Skip(events[start..idx].into())], ctx),
            }
        }
    }

    let mut groups = Vec::new();
    let mut state = State::Skip(0);
    let mut ctx = GroupingContext::default();

    for (idx, (_, event)) in events.iter().enumerate() {
        match event {
            // These block-level events force new groups. We do this
            // because we want to include these events in the group to
            // make the group self-contained.
            Event::Start(Tag::Paragraph | Tag::CodeBlock(..)) => {
                // A translatable group starts here.
                let mut next_groups;
                (next_groups, ctx) = state.into_groups(idx, events, ctx);
                groups.append(&mut next_groups);

                state = State::Translate(idx);
            }
            Event::End(TagEnd::Paragraph | TagEnd::CodeBlock) => {
                // A translatable group ends after `idx`.
                let idx = idx + 1;
                let mut next_groups;
                (next_groups, ctx) = state.into_groups(idx, events, ctx);
                groups.append(&mut next_groups);

                state = State::Skip(idx);
            }

            // Inline events start or continue a translating group.
            Event::Start(
                Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Link { .. }
                | Tag::Image { .. },
            )
            | Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image,
            )
            | Event::Text(_)
            | Event::Code(_)
            | Event::FootnoteReference(_)
            | Event::SoftBreak
            | Event::HardBreak => {
                // If we're currently skipping, then a new
                // translatable group starts here.
                if let State::Skip(_) = state {
                    let mut next_groups;
                    (next_groups, ctx) = state.into_groups(idx, events, ctx);
                    groups.append(&mut next_groups);

                    state = State::Translate(idx);
                }
            }

            Event::Html(s) | Event::InlineHtml(s) => {
                match directives::find(s) {
                    Some(directives::Directive::Skip) => {
                        // If in the middle of translation, finish it.
                        if let State::Translate(_) = state {
                            let mut next_groups;
                            (next_groups, ctx) = state.into_groups(idx, events, ctx);
                            groups.append(&mut next_groups);

                            // Restart translation: subtle but should be
                            // needed to handle the skipping of the rest of
                            // the inlined content.
                            state = State::Translate(idx);
                        }

                        ctx.skip_next_group = true;
                    }

                    Some(directives::Directive::Comment(comment)) => {
                        // If in the middle of translation, finish it.
                        if let State::Translate(_) = state {
                            let mut next_groups;
                            (next_groups, ctx) = state.into_groups(idx, events, ctx);
                            groups.append(&mut next_groups);

                            // Restart translation: subtle but should be
                            // needed to handle the skipping of the rest of
                            // the inlined content.
                            state = State::Translate(idx);
                        }

                        ctx.comments.push(comment);
                    }
                    _ => {
                        match event {
                            Event::Html(_) => {
                                // Otherwise, treat as a skipping group if this is a block level Html tag
                                if let State::Translate(_) = state {
                                    let mut next_groups;
                                    (next_groups, ctx) = state.into_groups(idx, events, ctx);
                                    groups.append(&mut next_groups);

                                    state = State::Skip(idx);
                                }
                            }
                            Event::InlineHtml(_) =>
                            // If we're currently skipping, then a new
                            // translatable group starts here.
                            {
                                if let State::Skip(_) = state {
                                    let mut next_groups;
                                    (next_groups, ctx) = state.into_groups(idx, events, ctx);
                                    groups.append(&mut next_groups);

                                    state = State::Translate(idx);
                                }
                            }
                            // this code is inside a match of Event::{Html|InlineHtml}, other types are not possible
                            _ => unreachable!(),
                        }
                    }
                }
            }

            // All other block-level events start or continue a
            // skipping group.
            _ => {
                if let State::Translate(_) = state {
                    let mut next_groups;
                    (next_groups, ctx) = state.into_groups(idx, events, ctx);
                    groups.append(&mut next_groups);

                    state = State::Skip(idx);
                }
            }
        }
    }

    match state {
        State::Translate(start) => groups.push(Group::Translate {
            events: events[start..].into(),
            comment: "".into(),
        }),
        State::Skip(start) => groups.push(Group::Skip(events[start..].into())),
    }

    groups
}

/// Returns true if the events appear to be a codeblock.
fn is_codeblock_group(events: &[(usize, Event<'_>)]) -> bool {
    matches!(
        events,
        [
            (_, Event::Start(Tag::CodeBlock(_))),
            ..,
            (_, Event::End(TagEnd::CodeBlock))
        ]
    )
}

/// Returns true if the scope should be translated.
fn is_translate_scope(x: Scope) -> bool {
    static SCOPE_STRING: OnceLock<Scope> = OnceLock::new();
    static SCOPE_COMMENT: OnceLock<Scope> = OnceLock::new();

    let scope_string = SCOPE_STRING.get_or_init(|| Scope::new("string").unwrap());
    let scope_comment = SCOPE_COMMENT.get_or_init(|| Scope::new("comment").unwrap());
    scope_string.is_prefix_of(x) || scope_comment.is_prefix_of(x)
}

/// Creates groups by checking codeblock with heuristic way.
fn heuristic_codeblock<'a>(
    events: &'a [(usize, Event<'_>)],
    mut ctx: GroupingContext,
) -> (Vec<Group<'a>>, GroupingContext) {
    let is_translate = match events {
        [(_, Event::Start(Tag::CodeBlock(_))), .., (_, Event::End(TagEnd::CodeBlock))] => {
            let (codeblock_text, _) = reconstruct_markdown(events, None);
            // Heuristic to check whether the codeblock nether has a
            // literal string nor a line comment.  We may actually
            // want to use a lexer here to make this more robust.
            codeblock_text.contains('"') || codeblock_text.contains("//")
        }
        _ => true,
    };

    if is_translate {
        (
            vec![Group::Translate {
                events: events.into(),
                comment: std::mem::take(&mut ctx.comments).join(" "),
            }],
            ctx,
        )
    } else {
        (vec![Group::Skip(events.into())], ctx)
    }
}

/// Creates groups by parsing codeblock.
fn parse_codeblock<'a>(
    events: &'a [(usize, Event<'_>)],
    mut ctx: GroupingContext,
) -> (Vec<Group<'a>>, GroupingContext) {
    // Language detection from language identifier of codeblock.
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    let ss = SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines);

    let syntax = if let (_, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(x)))) = &events[0] {
        ss.find_syntax_by_token(x.split(',').next().unwrap())
    } else {
        None
    };

    let Some(syntax) = syntax else {
        // If there is no language specifier, falling back to heuristic way.
        return heuristic_codeblock(events, ctx);
    };

    let mut ps = ParseState::new(syntax);
    let mut ret = vec![];

    for (idx, event) in events.iter().enumerate() {
        match event {
            (text_line, Event::Text(text)) => {
                let mut stack = ScopeStack::new();
                let mut stack_failure = false;

                let Ok(ops) = ps.parse_line(text, ss) else {
                    // If parse is failed, the text event should be translated.
                    ret.push(Group::Translate {
                        events: events[idx..idx + 1].into(),
                        comment: std::mem::take(&mut ctx.comments).join(" "),
                    });
                    continue;
                };

                let mut translate_events = vec![];
                let mut groups = vec![];

                for (range, op) in ScopeRangeIterator::new(&ops, text) {
                    if stack.apply(op).is_err() {
                        stack_failure = true;
                        break;
                    }

                    if range.is_empty() {
                        continue;
                    }

                    // Calculate line number of the range
                    let range_line = if range.start == 0 {
                        *text_line
                    } else {
                        text_line + text[0..range.start].lines().count() - 1
                    };

                    let text = &text[range];

                    // Whitespaces between translate texts should be added to translate
                    // group.
                    // So all whitespaces are added to the translate events buffer temporary,
                    // and the trailing whitespaces will be remvoed finally.
                    let is_whitespace = text.trim_matches(&[' ', '\t'] as &[_]).is_empty();

                    let is_translate = stack.scopes.iter().any(|x| is_translate_scope(*x));

                    if is_translate || (is_whitespace && !translate_events.is_empty()) {
                        translate_events.push((range_line, Event::Text(text.into())));
                    } else {
                        let whitespace_events = extract_trailing_whitespaces(&mut translate_events);
                        if !translate_events.is_empty() {
                            groups.push(Group::Translate {
                                events: std::mem::take(&mut translate_events),
                                comment: std::mem::take(&mut ctx.comments).join(" "),
                            });
                        }
                        if !whitespace_events.is_empty() {
                            groups.push(Group::Skip(whitespace_events));
                        }
                        groups.push(Group::Skip(vec![(range_line, Event::Text(text.into()))]));
                    }
                }

                let whitespace_events = extract_trailing_whitespaces(&mut translate_events);
                if !translate_events.is_empty() {
                    groups.push(Group::Translate {
                        events: std::mem::take(&mut translate_events),
                        comment: std::mem::take(&mut ctx.comments).join(" "),
                    });
                }
                if !whitespace_events.is_empty() {
                    groups.push(Group::Skip(whitespace_events));
                }

                if stack_failure {
                    // If stack operation is failed, the text event should be translated.
                    ret.push(Group::Translate {
                        events: events[idx..idx + 1].into(),
                        comment: std::mem::take(&mut ctx.comments).join(" "),
                    });
                } else {
                    ret.append(&mut groups);
                }
            }
            _ => {
                ret.push(Group::Skip(events[idx..idx + 1].into()));
            }
        }
    }
    (ret, ctx)
}

/// Extract trailing events which have whitespace only.
fn extract_trailing_whitespaces<'a>(buf: &mut Vec<(usize, Event<'a>)>) -> Vec<(usize, Event<'a>)> {
    let mut ret = vec![];

    while let Some(last) = buf.last() {
        match &last.1 {
            Event::Text(text) if text.as_ref().trim_matches(&[' ', '\t'] as &[_]).is_empty() => {
                let last = buf.pop().unwrap();
                ret.push(last);
            }
            _ => break,
        }
    }
    ret.reverse();
    ret
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
pub fn reconstruct_markdown<'a>(
    group: &[(usize, Event<'a>)],
    state: Option<State<'a>>,
) -> (String, State<'a>) {
    let events = group.iter().map(|(_, event)| event);
    let code_block_token_count = calculate_code_block_token_count(events.clone()).unwrap_or(3);
    let mut markdown = String::new();
    let options = Options {
        code_block_token_count,
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
    (String::from(markdown.trim_start_matches('\n')), new_state)
}

#[derive(Debug, PartialEq)]
pub struct ExtractedMessage {
    pub message: String,
    pub comment: String,
}
impl From<&str> for ExtractedMessage {
    fn from(s: &str) -> Self {
        ExtractedMessage {
            message: s.to_owned(),
            comment: "".into(),
        }
    }
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
pub fn extract_messages(document: &str) -> Vec<(usize, ExtractedMessage)> {
    let events = extract_events(document, None);
    let mut messages = Vec::new();
    let mut state = None;

    for group in group_events(&events) {
        match group {
            Group::Translate { events, comment } => {
                if let Some((lineno, _)) = events.first() {
                    let (text, new_state) = reconstruct_markdown(&events, state);
                    // Skip empty messages since they are special:
                    // they contains the PO file metadata.
                    if !text.trim().is_empty() {
                        messages.push((
                            *lineno,
                            ExtractedMessage {
                                message: text,
                                comment,
                            },
                        ));
                    }
                    state = Some(new_state);
                }
            }
            Group::Skip(events) => {
                let (_, new_state) = reconstruct_markdown(&events, state);
                state = Some(new_state);
            }
        }
    }

    messages
}

/// Trim `new_events` if they're wrapped in an unwanted paragraph.
///
/// If `new_events` is wrapped in a paragraph and `old_events` isn't,
/// then the paragraph is removed. This is useful when a text event
/// has been wrapped in a paragraph:
///
/// ```
/// use pulldown_cmark::{Event, Tag, TagEnd};
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
///         (1, Event::End(TagEnd::Paragraph)),
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
        [(_, Start(Paragraph)), inner @ .., (_, End(TagEnd::Paragraph))] => match old_events {
            [(_, Start(Paragraph)), .., (_, End(TagEnd::Paragraph))] => new_events,
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
            Group::Translate { events, .. } => {
                // Reconstruct the message.
                let (msgid, new_state) = reconstruct_markdown(&events, state.clone());
                let translated = catalog
                    .find_message(None, &msgid, None)
                    .filter(|msg| !msg.flags().is_fuzzy() && msg.is_translated())
                    .and_then(|msg| msg.msgstr().ok());
                match translated {
                    Some(msgstr) => {
                        // Generate new events for `msgstr`, taking
                        // care to trim away unwanted paragraphs.
                        translated_events.extend_from_slice(trim_paragraph(
                            &extract_events(msgstr, state),
                            &events,
                        ));
                    }
                    None => translated_events.extend_from_slice(&events),
                }
                // Advance the state.
                state = Some(new_state);
            }
            Group::Skip(events) => {
                // Copy the events unchanged to the output.
                translated_events.extend_from_slice(&events);
                // Advance the state.
                let (_, new_state) = reconstruct_markdown(&events, state);
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
    fn assert_extract_messages(document: &str, expected: &[(usize, &str)]) {
        assert_eq!(
            extract_messages(document)
                .iter()
                .map(|(lineno, msg)| (*lineno, &msg.message[..]))
                .collect::<Vec<_>>(),
            expected,
        );
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
                (1, End(TagEnd::Paragraph)),
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
                (1, End(TagEnd::Paragraph)),
            ]
        );
    }

    #[test]
    fn extract_events_heading() {
        assert_eq!(
            extract_events("# Foo Bar", None),
            vec![
                (
                    1,
                    Start(Tag::Heading {
                        level: H1,
                        id: None,
                        classes: vec![],
                        attrs: vec![]
                    })
                ),
                (1, Text("Foo Bar".into())),
                (1, End(TagEnd::Heading(H1))),
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
                (1, End(TagEnd::Item)),
                (1, End(TagEnd::List(false))),
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
                (1, End(TagEnd::Paragraph)),
            ]
        );
    }

    #[test]
    fn extract_events_comments() {
        assert_eq!(
            extract_events("<!-- mdbook-xgettext:skip -->\nHello", None),
            vec![
                (1, Start(HtmlBlock)),
                (1, Html("<!-- mdbook-xgettext:skip -->\n".into())),
                (1, End(TagEnd::HtmlBlock)),
                (2, Start(Paragraph)),
                (2, Text("Hello".into())),
                (2, End(TagEnd::Paragraph)),
            ]
        );
    }

    #[test]
    fn extract_messages_empty() {
        assert_extract_messages("", &[]);
    }

    #[test]
    fn extract_messages_keep_empty_inline_html() {
        // Keep inline html tags
        assert_extract_messages("<span></span>", &[(1, "<span></span>")]);
    }

    #[test]
    fn extract_messages_keep_whitespace_inline_html() {
        // span is an inline html tag so even whitespace is kept as is
        assert_extract_messages("<span>  </span>", &[(1, "<span>  </span>")]);
    }

    #[test]
    fn extract_messages_ignore_whitespace_only_block_html() {
        // Whitespace in block level html tags is ignored
        assert_extract_messages("<p>  </p>", &[]);
    }

    #[test]
    fn extract_messages_single_line() {
        assert_extract_messages("This is a paragraph.", &[(1, "This is a paragraph.")]);
    }

    #[test]
    fn extract_messages_simple() {
        assert_extract_messages(
            "This is\n\
             the first\n\
             paragraph.🦀\n\
             \n\
             Second paragraph.",
            &[
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
            &[(4, "This is the first paragraph.")],
        );
    }

    #[test]
    fn extract_messages_trailing_newlines() {
        assert_extract_messages(
            "This is\n\
             a paragraph.\n\
             \n\
             \n",
            &[(1, "This is a paragraph.")],
        );
    }

    #[test]
    fn extract_messages_styled_text() {
        // The parser normalizes "*emphasis*" to "_emphasis_" and
        // "__strong emphasis__" to "**strong emphasis**".
        assert_extract_messages(
            "**This** __~~message~~__ _has_ `code` *style*\n",
            &[(1, "**This** **~~message~~** _has_ `code` _style_")],
        );
    }

    #[test]
    fn extract_messages_inline_html() {
        // Inline HTML tag is kept as is in the translation.
        assert_extract_messages(
            "Hi from <span dir=\"ltr\">Rust</div>",
            &[(1, "Hi from <span dir=\"ltr\">Rust</div>")],
        );
    }

    #[test]
    fn extract_messages_block_html() {
        // block level HTML tag is skipped, but text inside is extracted.
        assert_extract_messages(
            "<div class=\"warning\">\n\
            \n\
            Beware of the dog!\n\
            \n\
            </div>",
            &[(3, "Beware of the dog!")],
        );
    }

    #[test]
    fn extract_messages_mixed_html() {
        // block level HTML tag is skipped, but text inside is extracted with inline html as is.
        assert_extract_messages(
            "<div>\n\
            \n\
            Hi from <span dir=\"ltr\">Rust</span>\n\
            \n\
            </div>",
            &[(3, "Hi from <span dir=\"ltr\">Rust</span>")],
        );
    }

    #[test]
    fn extract_messages_inline_link() {
        assert_extract_messages(
            "See [this page](https://example.com) for more info.",
            &[(1, "See [this page](https://example.com) for more info.")],
        );
    }

    #[test]
    fn extract_messages_reference_link() {
        assert_extract_messages(
            "See [this page][1] for more info.\n\n\
             [1]: https://example.com",
            // The parser expands reference links on the fly.
            &[(1, "See [this page](https://example.com) for more info.")],
        );
    }

    #[test]
    fn extract_messages_collapsed_link() {
        // We make the parser expand collapsed links on the fly.
        assert_extract_messages(
            "Click [here][]!\n\n\
             [here]: http://example.net/",
            &[(1, "Click [here](http://example.net/)!")],
        );
    }

    #[test]
    fn extract_messages_shortcut_link() {
        assert_extract_messages(
            "Click [here]!\n\n\
             [here]: http://example.net/",
            &[(1, "Click [here](http://example.net/)!")],
        );
    }

    #[test]
    fn extract_messages_autolink() {
        assert_extract_messages(
            "Visit <http://example.net>!",
            &[(1, "Visit <http://example.net>!")],
        );
    }

    #[test]
    fn extract_messages_email() {
        assert_extract_messages(
            "Contact <info@example.net>!",
            &[(1, "Contact <info@example.net>!")],
        );
    }

    #[test]
    fn extract_messages_broken_reference_link() {
        // A reference link without the corresponding link definition
        // results in an escaped link.
        //
        // See `SourceMap::extract_messages` for a more complex
        // approach which can work around this in some cases.
        assert_extract_messages("[foo][unknown]", &[(1, r"\[foo\]\[unknown\]")]);
    }

    #[test]
    fn extract_messages_footnotes() {
        assert_extract_messages(
            "
The document[^1] text.

[^1]: The footnote text.
",
            &[
                (2, "The document[^1] text."), //
                (4, "The footnote text."),
            ],
        );
    }

    #[test]
    fn extract_messages_block_quote() {
        assert_extract_messages(
            r"One of my favorite quotes is:

> Don't believe everything you read on the Internet.
>
> I didn't say this second part, but I needed a paragraph for testing.

--Abraham Lincoln
",
            &[
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
            &[
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
            &[
                (1, "Preamble"),
                (
                    3,
                    "// Example:\n",
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
            &[
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
            &[
                (1, "Preamble"),
                (6, "// FIXME: do something here!\n"),
                (10, "Postamble"),
            ],
        );
    }

    #[test]
    fn extract_messages_code_block_with_block_comment() {
        assert_extract_messages(
            "```rust\n\
            /* block comment\n\
             * /* nested block comment\n\
             * */\n\
             * \n\
             * \n\
             * \n\
             * */\n\
            ```\n",
            &[(
                2,
                "/* block comment\n* /* nested block comment\n* */\n* \n* \n* \n* */",
            )],
        );
    }

    #[test]
    fn extract_messages_code_block_with_continuous_line_comments() {
        assert_extract_messages(
            r"```rust
// continuous
// line
// comments
{
    // continuous
    // line
    // comments
    let a = 1; // single line comment
    let b = 1; // single line comment
}
```",
            &[
                (2, "// continuous\n// line\n// comments\n"),
                (6, "// continuous\n    // line\n    // comments\n"),
                (9, "// single line comment\n"),
                (10, "// single line comment\n"),
            ],
        );
    }

    #[test]
    fn extract_messages_multi_language_code_blocks() {
        assert_extract_messages(
            r#"```c
// C
'C'; "C";
```
```html
<!-- HTML
HTML -->
```
```javascript
`JavaScript`
```
```ruby
# Ruby
```"#,
            &[
                (2, "// C\n'C'"),
                (3, "\"C\""),
                (6, "<!-- HTML\nHTML -->"),
                (10, "`JavaScript`"),
                (13, "# Ruby\n"),
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
            &[
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
            &[
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
            &[
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
            &[
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
            r"* Item 1.
* Item 2,
  two lines.

  * Sub 1.
  * Sub 2.
",
            &[
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
            r"Some text
# Headline News🦀

* A
* List

## Subheading
",
            &[
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
            r"```bob
// BOB
```

<details>

* Blah blah

</details>
",
            &[
                (1, "```bob\n// BOB\n```"), //
                (7, "Blah blah"),
            ],
        );
    }

    #[test]
    fn extract_messages_backslashes() {
        // Demonstrate how a single backslash in the Markdown becomes
        // a backslash-escaped backslash when we extract the text.
        // This is consistent with the CommonMark spec:
        // https://spec.commonmark.org/0.30/#backslash-escapes.
        // However, it causes problems for LaTeX preprocessors:
        // https://github.com/google/mdbook-i18n-helpers/issues/105.
        assert_extract_messages(
            r"
$$
\sum_{n=1}^{\infty} 2^{-n} = 1
$$
",
            &[(2, r"$$ \\sum\_{n=1}^{\infty} 2^{-n} = 1 $$")],
        );
    }

    #[test]

    fn extract_messages_skip_simple() {
        assert_extract_messages(
            r"<!-- mdbook-xgettext:skip -->

This is a paragraph.",
            &[],
        );
    }

    #[test]
    fn extract_messages_skip_next_paragraph_ok() {
        assert_extract_messages(
            r"<!-- mdbook-xgettext:skip -->
This is a paragraph.

This should be translated.
",
            &[(4, "This should be translated.")],
        );
    }

    #[test]
    fn extract_messages_skip_next_codeblock() {
        assert_extract_messages(
            r"<!-- mdbook-xgettext:skip -->
```
def f(x): return x * x
```
This should be translated.
",
            &[(5, "This should be translated.")],
        );
    }

    #[test]
    fn extract_messages_skip_back_to_back() {
        assert_extract_messages(
            r"<!-- mdbook-xgettext:skip -->
```
def f(x): return x * x
```
<!-- mdbook-xgettext:skip -->
This should not translated.

But *this* should!
",
            &[(8, "But _this_ should!")],
        );
    }

    #[test]
    fn extract_messages_block_html_skip() {
        // The comment is a block level html tag.
        assert_extract_messages(
            "<!-- mdbook-xgettext:skip -->\n\
            This is ignored\n\
            \n\
            but this is not",
            &[(4, "but this is not")],
        );
    }

    #[test]
    fn extract_messages_inline_html_skips() {
        // The comment is an inline html tag.
        assert_extract_messages(
            "
this should be translated <!-- mdbook-xgettext:skip --> but not this.
... nor this.

But *this* should!",
            &[(2, "this should be translated "), (5, "But _this_ should!")],
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
            &[(2, "A"), (5, "C")],
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
            &[(2, "A"), (7, "C")],
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
            &[(2, "A")],
        );
    }

    #[test]
    fn extract_messages_inline_skip_to_end_of_block() {
        assert_extract_messages(
            "foo <!-- mdbook-xgettext:skip --> **bold** bar
still skipped

not-skipped",
            &[(1, "foo "), (4, "not-skipped")],
        );
    }

    #[test]
    fn extract_messages_automatic_skipping_nontranslatable_codeblocks_simple() {
        assert_extract_messages(
            r"
```python
def g(x):
  this_should_be_skipped_no_strings_or_comments()
```
",
            &[],
        );
    }

    #[test]
    fn extract_messages_automatic_skipping_nontranslatable_codeblocks() {
        assert_extract_messages(
            r#"
```python
def f(x):
  print("this should be translated")
```


```python
def g(x):
  but_this_should_not()
```
"#,
            &[(4, "\"this should be translated\"")],
        );
    }

    #[test]
    fn extract_messages_without_language_specifier() {
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
            &[(
                2,
                "```\ndef f(x):\n  print(\"this should be translated\")\n```",
            )],
        );
    }

    #[test]
    fn extract_messages_codeblock_in_codeblock() {
        assert_extract_messages(
            r#"
````
```
// codeblock in codeblock
```
````
"#,
            &[(2, "````\n```\n// codeblock in codeblock\n```\n````")],
        );
    }

    #[test]
    fn extract_message_comments() {
        assert_eq!(
            extract_messages(
                "
<!-- mdbook-xgettext:comment: first comment! -->
Hello world!
"
            ),
            vec![(
                3,
                ExtractedMessage {
                    message: "Hello world!".into(),
                    comment: "first comment!".into(),
                }
            )]
        );
    }

    #[test]
    fn extract_message_comments_multiple_joined() {
        assert_eq!(
            extract_messages(
                "
<!-- mdbook-xgettext:comment: this is a test -->
<!-- mdbook-xgettext:comment: of a comment that spans. -->
Greetings!
"
            ),
            vec![(
                4,
                ExtractedMessage {
                    message: "Greetings!".into(),
                    comment: "this is a test of a comment that spans.".into(),
                }
            )]
        );
    }

    #[test]
    fn extract_message_multiple_comments() {
        assert_eq!(
            extract_messages(
                "
before-no-comment

<!-- mdbook-xgettext:comment: another -->
Hello again, this is some text
with a comment on it.

<!-- mdbook-xgettext:comment: one more comment. -->
after

after-no-comment
"
            ),
            vec![
                (
                    2,
                    ExtractedMessage {
                        message: "before-no-comment".into(),
                        comment: "".into(),
                    }
                ),
                (
                    5,
                    ExtractedMessage {
                        message: "Hello again, this is some text with a comment on it.".into(),
                        comment: "another".into(),
                    }
                ),
                (
                    9,
                    ExtractedMessage {
                        message: "after".into(),
                        comment: "one more comment.".into(),
                    }
                ),
                (
                    11,
                    ExtractedMessage {
                        message: "after-no-comment".into(),
                        comment: "".into(),
                    }
                ),
            ]
        );
    }

    #[test]
    fn extract_message_comments_on_codeblock() {
        assert_eq!(
            extract_messages(
                r#"
<!-- mdbook-xgettext:comment: greetings! -->
```python
print("Hello world")
```
"#
            ),
            vec![(
                4,
                ExtractedMessage {
                    message: "\"Hello world\"".into(),
                    comment: "greetings!".into(),
                }
            ),]
        );
    }
}
