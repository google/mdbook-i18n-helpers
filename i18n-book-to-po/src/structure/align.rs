use std::vec;

use mdbook_i18n_helpers::reconstruct_markdown;
use pulldown_cmark::Event;

use crate::structure::{
    diff::diff_structure,
    types::{AlignAction, CmarkEvent, DiffAlgorithm},
};

/// generate a sentence structure based on the amount of sentences (separated by dots).
/// This is splitting by "." and replacing these elements by a Vector of Sentence Elements
fn generate_sentence_structure(text: &str) -> Vec<CmarkEvent> {
    text.split(".")
        .into_iter()
        .filter_map(|sentence| {
            if sentence.is_empty() {
                None
            } else {
                Some(CmarkEvent::SentenceElement)
            }
        })
        .collect()
}

/// Parse the structure of the provided cmark events and return a Markdown structure
/// It leverages additional sentence elements as structural elements if feature_sentence is set
fn parse_structure(
    markdown: &Vec<pulldown_cmark::Event<'_>>,
    feature_sentence: bool,
) -> Vec<CmarkEvent> {
    let structure: Vec<_> = markdown
        .iter()
        .flat_map(|event| {
            if feature_sentence {
                // if sentences should be used to align the documents, split the
                // Text Elements into sentences.
                if let pulldown_cmark::Event::Text(text) = event {
                    // prepend the sentence elements with the Text variant
                    let mut result = vec![(event).into()];
                    result.extend(generate_sentence_structure(&text));
                    return result;
                }
            };
            vec![(event).into()]
        })
        .collect();

    return structure;
}

/// normalize the event stream
/// This is done to avoid issues with Softbreak and similar events that might be in different
/// positions in the translation.
/// Currently this removes SoftBreaks and merges Text blocks after SoftBreaks to get a normalized structure
fn normalize_events(events: Vec<pulldown_cmark::Event>) -> Vec<pulldown_cmark::Event> {
    let mut normalized_events = Vec::new();
    let mut removed_softbreak = false;
    for event in events {
        match event {
            pulldown_cmark::Event::SoftBreak => removed_softbreak = true,
            pulldown_cmark::Event::Text(text) => {
                // if a softbreak was just removed and we have a text event, merge it with
                // a potential text element in front of the softbreak
                if removed_softbreak {
                    if let Some(pulldown_cmark::Event::Text(prev_text)) =
                        normalized_events.last_mut()
                    {
                        // merge text events (with space as this is a soft break)
                        *prev_text = format!("{} {}", prev_text, text).into();
                    } else {
                        // add the text event unmodified
                        normalized_events.push(pulldown_cmark::Event::Text(text));
                    }
                } else {
                    // add the text event unmodified
                    normalized_events.push(pulldown_cmark::Event::Text(text));
                }
            }
            _ => {
                removed_softbreak = false;
                normalized_events.push(event);
            }
        }
    }
    normalized_events
}

/// helper function to read the markdown events. This can be replaced by
/// mdbook::utils::new_cmark_parser() once the version of pulldown-cmark is up-to-date
/// Then replace this call with
/// `mdbook::utils::new_cmark_parser(&content, false).collect();`
fn read_structure(content: &str) -> anyhow::Result<Vec<pulldown_cmark::Event<'_>>> {
    // This is a using pulldown-cmark 0.10...
    // let parser = mdbook::utils::new_cmark_parser(&content, false);
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    opts.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    opts.insert(pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES);

    Ok(pulldown_cmark::Parser::new_ext(&content, opts).collect())
}

/// apply the diff to align the markdown events.
/// if an element is not available in the other document, this will output None in its place
fn align_markdown_events<'a>(
    diff: Vec<AlignAction>,
    source: Vec<Event<'a>>,
    translated: Vec<Event<'a>>,
) -> (Vec<Option<Event<'a>>>, Vec<Option<Event<'a>>>) {
    // small hack to make the data structure better accessible as pop is easy on vec
    let mut reversed_source = source.into_iter().rev().collect::<Vec<_>>();
    let mut reversed_translated = translated.into_iter().rev().collect::<Vec<_>>();

    // These will store the aligned source and translation events wrapped in Some
    // if something is missing in one stream, a None will be placed
    let mut aligned_source = vec![];
    let mut aligned_translated = vec![];

    for action in diff {
        match action {
            AlignAction::Source(_data_) => {
                aligned_source.push(reversed_source.pop());
                aligned_translated.push(None);
            }
            AlignAction::Translation(_data) => {
                aligned_source.push(None);
                aligned_translated.push(reversed_translated.pop());
            }
            AlignAction::Both(_data) => {
                aligned_source.push(reversed_source.pop());
                aligned_translated.push(reversed_translated.pop());
            }
            AlignAction::Different(_source, _translation) => {
                // discard these elements
                reversed_source.pop();
                reversed_translated.pop();
                // and show this with None
                aligned_source.push(None);
                aligned_translated.push(None);
            }
        }
    }
    // both streams need to be empty, otherwise this would indicate a bug
    assert!(reversed_source.is_empty());
    assert!(reversed_translated.is_empty());
    // both aligned streams should be equal in length
    assert_eq!(aligned_source.len(), aligned_translated.len());
    (aligned_source.clone(), aligned_translated.clone())
}

/// filter the source and translation files to only return elements that are available in both
fn minimize_aligned_events<'a>(
    source: Vec<Option<Event<'a>>>,
    translated: Vec<Option<Event<'a>>>,
) -> (Vec<Event<'a>>, Vec<Event<'a>>) {
    source
        .into_iter()
        .zip(translated)
        .filter_map(|(s, t)| {
            if s.is_some() && t.is_some() {
                Some((s.unwrap(), t.unwrap()))
            } else {
                None
            }
        })
        .unzip()
}

/// this is a debug variant of minimize_aligned_events() that returns all events on both sides
/// that don't have a pendant in the other document. This is mostly useful for debugging if the
/// markdown cannot be properly reconstructed.
fn debug_get_unaligned_events<'a>(
    source: Vec<Option<Event<'a>>>,
    translated: Vec<Option<Event<'a>>>,
) -> (Vec<Option<Event<'a>>>, Vec<Option<Event<'a>>>) {
    source
        .into_iter()
        .zip(translated)
        .filter_map(|(s, t)| {
            if s.is_none() || t.is_none() {
                Some((s, t))
            } else {
                None
            }
        })
        .unzip()
}

/// This is the main worker function.
/// It aligns two markdown documents based on their structure.
///
/// This function has the steps:
/// - read markdown structure from both documents (read_structure)
/// - prepare both event streams by removing content from structure elements (parse_structure)
/// - diff the structural elements (without content)
/// - apply the diff to both event streams that still contain content (align_markdown_events)
/// - minimize the aligned aligned markdown event streams by removing everything that is not in both (minimize_aligned_events)
/// - reconstruct the markdown from the minimized streams and return both documents (reconstruct_markdown)
pub fn align_markdown_docs(
    source: &str,
    translation: &str,
    normalize: bool,
    diff_algorithm: &DiffAlgorithm,
) -> anyhow::Result<(String, String)> {
    let source_events = read_structure(source)?;
    let translated_events = read_structure(translation)?;
    // remove some events if normalization is used that are not needed for alignment, e.g. soft breaks
    let source_events = if normalize {
        normalize_events(source_events)
    } else {
        source_events
    };
    let translated_events = if normalize {
        normalize_events(translated_events)
    } else {
        translated_events
    };

    let source_structure = parse_structure(&source_events, false);
    let translated_structure = parse_structure(&translated_events, false);

    let diff = diff_structure(&source_structure, &translated_structure, diff_algorithm);

    let (aligned_source, aligned_translated) =
        align_markdown_events(diff, source_events, translated_events);

    let (minimized_source, minimized_translated) =
        minimize_aligned_events(aligned_source.clone(), aligned_translated.clone());

    let reconstructed_source = reconstruct_markdown(
        &minimized_source
            .iter()
            .map(|event| (0_usize, (*event).clone()))
            .collect::<Vec<_>>(),
        None,
    );
    if let Err(e) = reconstructed_source {
        println!("Error reconstructing source markdown: {:?}", e);
        dbg!(&aligned_source);
        dbg!(&aligned_translated);
        dbg!(debug_get_unaligned_events(
            aligned_source,
            aligned_translated
        ));
        return Err(e.into());
    }
    let reconstructed_source = reconstructed_source.unwrap();

    let reconstructed_translated = reconstruct_markdown(
        &minimized_translated
            .iter()
            .map(|event| (0_usize, (*event).clone()))
            .collect::<Vec<_>>(),
        None,
    );
    if let Err(e) = reconstructed_translated {
        println!("Error reconstructing translated markdown: {:?}", e);
        dbg!(&aligned_source);
        dbg!(&aligned_translated);
        dbg!(debug_get_unaligned_events(
            aligned_source,
            aligned_translated
        ));
        return Err(e.into());
    }
    let reconstructed_translated = reconstructed_translated.unwrap();

    Ok((reconstructed_source.0, reconstructed_translated.0))
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::structure::diff::diff_structure;
    use crate::structure::types::{
        AlignAction, CmarkEvent, CmarkTagEnd, CmarkTagStart, DiffAlgorithm,
    };
    use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};

    use crate::structure::align::{
        align_markdown_docs, align_markdown_events, minimize_aligned_events, parse_structure,
        read_structure,
    };

    /// test reading text into a pulldown_cmark::Event vector
    #[test]
    fn test_read_structure() {
        let markdown_doc = "# Title 1
First paragraph. Second sentence.

Second paragraph. 2nd sentence. 3rd sentence.
        ";
        let got_markdown_events: Vec<pulldown_cmark::Event<'_>> =
            read_structure(markdown_doc).unwrap();
        let want_markdown_events = [
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                id: None,
                classes: vec![],
                attrs: vec![],
            }),
            Event::Text(Cow::Borrowed("Title 1").into()),
            Event::End(TagEnd::Heading(HeadingLevel::H1)),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("First paragraph. Second sentence.").into()),
            Event::End(TagEnd::Paragraph),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Second paragraph. 2nd sentence. 3rd sentence.").into()),
            Event::End(TagEnd::Paragraph),
        ];
        assert_eq!(got_markdown_events, want_markdown_events)
    }

    /// test parsing the structure from text (without content and without the sentence feature).
    /// reading the structure is assumed to be correct in this test.
    #[test]
    fn test_parse_structure_without_sentence() {
        let markdown_doc = "# Title 1
First paragraph. Second sentence.

Second paragraph. 2nd sentence. 3rd sentence.
        ";
        let events = read_structure(markdown_doc).unwrap();
        let got_markdown_events = parse_structure(&events, false);
        let want_markdown_events = [
            CmarkEvent::Start(CmarkTagStart::Heading {
                level: HeadingLevel::H1,
            }),
            CmarkEvent::Text,
            CmarkEvent::End(CmarkTagEnd::Heading(HeadingLevel::H1)),
            CmarkEvent::Start(CmarkTagStart::Paragraph),
            CmarkEvent::Text,
            CmarkEvent::End(CmarkTagEnd::Paragraph),
            CmarkEvent::Start(CmarkTagStart::Paragraph),
            CmarkEvent::Text,
            CmarkEvent::End(CmarkTagEnd::Paragraph),
        ];
        assert_eq!(got_markdown_events, want_markdown_events);
    }

    /// test parsing the structure from text (without content but with the sentence feature)
    /// reading the structure is assumed to be correct in this test.
    #[test]
    fn test_parse_structure_with_sentence() {
        let markdown_doc = "# Title 1
First paragraph. Second sentence.

Second paragraph. 2nd sentence. 3rd sentence.
        ";
        let events = read_structure(markdown_doc).unwrap();
        let got_markdown_events = parse_structure(&events, true);
        let want_markdown_events = [
            CmarkEvent::Start(CmarkTagStart::Heading {
                level: HeadingLevel::H1,
            }),
            CmarkEvent::Text,
            CmarkEvent::SentenceElement,
            CmarkEvent::End(CmarkTagEnd::Heading(HeadingLevel::H1)),
            CmarkEvent::Start(CmarkTagStart::Paragraph),
            CmarkEvent::Text,
            CmarkEvent::SentenceElement,
            CmarkEvent::SentenceElement,
            CmarkEvent::End(CmarkTagEnd::Paragraph),
            CmarkEvent::Start(CmarkTagStart::Paragraph),
            CmarkEvent::Text,
            CmarkEvent::SentenceElement,
            CmarkEvent::SentenceElement,
            CmarkEvent::SentenceElement,
            CmarkEvent::End(CmarkTagEnd::Paragraph),
        ];
        assert_eq!(got_markdown_events, want_markdown_events);
    }

    /// test if two documents with the same structure but different content are considered equal
    #[test]
    fn test_equal_structure() {
        let original_doc = "# Title 1
First paragraph. Second sentence.

Second paragraph. 2nd sentence. 3rd sentence.
        ";
        let translated_doc = "# Foobar et 1
Bla Baz. Foobar bar 42.

Baz Bla. Lorem. Ipsum.
        ";
        let original_structure = parse_structure(&read_structure(original_doc).unwrap(), true);
        let translated_structure = parse_structure(&read_structure(translated_doc).unwrap(), true);

        assert_eq!(original_structure, translated_structure);
    }

    /// test if the diff between two markdown source texts generates correct AlignActions.
    /// Some text in the source is not in the translation and vice versa. This should show
    /// up in the AlignActions
    #[test]
    fn test_diff_structure() {
        let original_doc = "# Title 1
translated sentence.

untranslated sentence

# Title 2
        ";
        let translated_doc = "# Title 1
Bla Baz. Foobar bar 42.

# Title 2

new sentence";
        let original_structure = parse_structure(&read_structure(original_doc).unwrap(), false);
        let translated_structure = parse_structure(&read_structure(translated_doc).unwrap(), false);

        let got_diff = diff_structure(
            &original_structure,
            &translated_structure,
            &DiffAlgorithm::default(),
        );
        let want_diff = [
            AlignAction::Both(CmarkEvent::Start(CmarkTagStart::Heading {
                level: HeadingLevel::H1,
            })),
            AlignAction::Both(CmarkEvent::Text),
            AlignAction::Both(CmarkEvent::End(CmarkTagEnd::Heading(HeadingLevel::H1))),
            AlignAction::Both(CmarkEvent::Start(CmarkTagStart::Paragraph)),
            AlignAction::Both(CmarkEvent::Text),
            AlignAction::Both(CmarkEvent::End(CmarkTagEnd::Paragraph)),
            AlignAction::Source(CmarkEvent::Start(CmarkTagStart::Paragraph)),
            AlignAction::Source(CmarkEvent::Text),
            AlignAction::Source(CmarkEvent::End(CmarkTagEnd::Paragraph)),
            AlignAction::Both(CmarkEvent::Start(CmarkTagStart::Heading {
                level: HeadingLevel::H1,
            })),
            AlignAction::Both(CmarkEvent::Text),
            AlignAction::Both(CmarkEvent::End(CmarkTagEnd::Heading(HeadingLevel::H1))),
            AlignAction::Translation(CmarkEvent::Start(CmarkTagStart::Paragraph)),
            AlignAction::Translation(CmarkEvent::Text),
            AlignAction::Translation(CmarkEvent::End(CmarkTagEnd::Paragraph)),
        ];
        assert_eq!(got_diff, want_diff);
    }

    /// test if two streams of pulldown_cmark::Events are diffed correctly
    /// and aligned properly. The structure will be a vector of of Option<Events>
    /// with None being inserted in a stream if an event in the other stream is not
    /// available in it
    #[test]
    fn test_align_markdown_events() {
        let translated_a = vec![
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                id: None,
                classes: vec![],
                attrs: vec![],
            }),
            Event::Text(Cow::Borrowed("Title 1").into()),
            Event::End(TagEnd::Heading(HeadingLevel::H1)),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("to translate sentence").into()),
            Event::End(TagEnd::Paragraph),
        ];
        let untranslated_paragraph = vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("untranslated sentence").into()),
            Event::End(TagEnd::Paragraph),
        ];
        let translated_b = vec![
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                id: None,
                classes: vec![],
                attrs: vec![],
            }),
            Event::Text(Cow::Borrowed("Title 2").into()),
            Event::End(TagEnd::Heading(HeadingLevel::H1)),
        ];
        let new_paragraph_in_translation = vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("new sentence").into()),
            Event::End(TagEnd::Paragraph),
        ];

        // this assumes that there is a new untranslated pragraph in between
        let original_events = [&translated_a, &untranslated_paragraph, &translated_b]
            .into_iter()
            .flatten()
            .cloned()
            .collect();

        // the untranslated paragraph is missing but a new sentence was added by the translator
        let translated_events = [&translated_a, &translated_b, &new_paragraph_in_translation]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        let original_structure = parse_structure(&original_events, false);
        let translated_structure = parse_structure(&translated_events, false);
        let diff = diff_structure(
            &original_structure,
            &translated_structure,
            &DiffAlgorithm::default(),
        );

        let (got_aligned_source, got_aligned_translated) =
            align_markdown_events(diff, original_events, translated_events);

        let want_aligned_source: Vec<_> = translated_a
            .iter()
            .map(|e| Some(e.clone()))
            .chain(untranslated_paragraph.iter().map(|e| Some(e.clone())))
            .chain(translated_b.iter().map(|e| Some(e.clone())))
            .chain(new_paragraph_in_translation.iter().map(|_| None))
            .collect();

        let want_aligned_translated: Vec<_> = translated_a
            .iter()
            .map(|e| Some(e.clone()))
            .chain(untranslated_paragraph.iter().map(|_| None))
            .chain(translated_b.iter().map(|e| Some(e.clone())))
            .chain(new_paragraph_in_translation.iter().map(|e| Some(e.clone())))
            .collect();

        assert_eq!(got_aligned_source, want_aligned_source);
        assert_eq!(got_aligned_translated, want_aligned_translated);
    }

    /// E2E test for
    /// - reading the pulldown_cmark::Events from text
    /// - converting into content-less CmarkEvents to get the raw structure
    /// - diff the structure and generate a stream of AlignActions
    /// - align the pulldown_cmark::Events with the created AlignActions
    /// - minimize these aligned events (keep only Events that occur in both docs)
    /// - compare against a generated Event stream from a known good document
    #[test]
    fn test_align_markdown_events_full() {
        let original_doc = "# Title 1
translated sentence.

untranslated sentence

# Title 2
        ";
        let translated_doc = "# Title 1
Bla Baz. Foobar bar 42.

# Title 2

new sentence";
        let original_events = read_structure(original_doc).unwrap();
        let translated_events = read_structure(translated_doc).unwrap();
        let original_structure = parse_structure(&original_events, false);
        let translated_structure = parse_structure(&translated_events, false);
        let diff = diff_structure(
            &original_structure,
            &translated_structure,
            &DiffAlgorithm::default(),
        );

        let (got_aligned_source, got_aligned_translated) =
            align_markdown_events(diff, original_events, translated_events);

        let (got_aligned_source, got_aligned_translated) =
            minimize_aligned_events(got_aligned_source, got_aligned_translated);

        let want_aligned_source = read_structure(
            "# Title 1
translated sentence.

# Title 2",
        )
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();
        let want_aligned_translated = read_structure(
            "# Title 1
Bla Baz. Foobar bar 42.

# Title 2",
        )
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();

        assert_eq!(got_aligned_source, want_aligned_source);
        assert_eq!(got_aligned_translated, want_aligned_translated);
    }

    /// test minimizing the aligned event streams.
    /// This should emit only Events that occur in both streams
    #[test]
    fn test_minimize_aligned_events() {
        let aligned_source = vec![
            Some(Event::Text(Cow::Borrowed("translated sentence").into())),
            Some(Event::Text(Cow::Borrowed("untranslated sentence").into())),
            None,
        ];
        let aligned_translated = vec![
            Some(Event::Text(Cow::Borrowed("translated sentence").into())),
            None,
            Some(Event::Text(Cow::Borrowed("new sentence").into())),
        ];
        let (got_aligned_source, got_aligned_translated) =
            minimize_aligned_events(aligned_source, aligned_translated);

        let want = [Event::Text(Cow::Borrowed("translated sentence").into())];

        assert_eq!(got_aligned_source, want);
        assert_eq!(got_aligned_translated, want);
    }

    /// full E2E test that is creating fully aligned markdown docs
    /// containing only content that is available in both documents.
    #[test]
    fn test_align_markdown_docs() {
        // original has one sentence more than translation in section 1
        // but translation has an added sentence in section 2
        let original_doc = "# source title 1
translated source sentence.

untranslated source sentence

# source title 2

translated source sentence";
        let translated_doc = "# target title 1

translated target sentence

# target title 2

translated target sentence

new target sentence";

        let (got_source, got_translated) = align_markdown_docs(
            original_doc,
            translated_doc,
            true,
            &DiffAlgorithm::default(),
        )
        .unwrap();

        // they should both have only the translated sentences
        let want_source = "# source title 1

translated source sentence.

# source title 2

translated source sentence";
        let want_translated = "# target title 1

translated target sentence

# target title 2

translated target sentence";

        assert_eq!(got_source, want_source);
        assert_eq!(got_translated, want_translated);
    }
}
