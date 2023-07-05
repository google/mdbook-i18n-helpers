#![no_main]

use libfuzzer_sys::fuzz_target;
use mdbook_i18n_helpers::{extract_events, group_events, Group};
use pretty_assertions::assert_eq;

fuzz_target!(|text: String| {
    let events = extract_events(&text, None);
    let flattened_groups = group_events(&events)
        .into_iter()
        .flat_map(|group| match group {
            Group::Translate(events) | Group::Skip(events) => events,
        })
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(events, flattened_groups);
});
