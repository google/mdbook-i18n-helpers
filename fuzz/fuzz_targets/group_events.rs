#![no_main]

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::{extract_events, group_events, reconstruct_markdown, Group};
use pretty_assertions::assert_eq;

fuzz_target!(|text: String| {
    let events = extract_events(&text, None);
    let flattened_groups = group_events(&events)
        .into_iter()
        .flat_map(|group| match group {
            Group::Translate { events, .. } | Group::Skip(events) => events,
        })
        .collect::<Vec<_>>();

    // Comparison through markdown text to detect missing text.
    // Events can't be compared directly because `group_events`
    // may split a event into some events.
    let text_from_events = reconstruct_markdown(&events, None);
    let text_from_groups = reconstruct_markdown(&flattened_groups, None);

    assert_eq!(text_from_events, text_from_groups);
});
