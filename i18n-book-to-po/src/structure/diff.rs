use lcs_diff::DiffResult;

use crate::structure::types::{AlignAction, CmarkEvent, DiffAlgorithm};

/// this diffs the structure in how the original needs to be modified in order to create the translation.
/// lcs_diff:diff() already does the job, but we transform this to a better understandable datastructure
fn diff_structure_lcs(source: &[CmarkEvent], translated: &[CmarkEvent]) -> Vec<AlignAction> {
    lcs_diff::diff(translated, source)
        .into_iter()
        .map(|change| {
            match change {
                // this element does not exist in the original
                DiffResult::Removed(diff_element) => AlignAction::Translation(diff_element.data),
                // both sides are equal
                DiffResult::Common(diff_element) => AlignAction::Both(diff_element.data),
                // this element does not exist in the translation
                DiffResult::Added(diff_element) => AlignAction::Source(diff_element.data),
            }
        })
        .collect()
}

/// this diffs the structure in how the original needs to be modified in order to create the translation.
/// We use the global alignment algorithm NeedlemanWunsch and transform the result into a understandable datastructure
fn diff_structure_seal(source: &[CmarkEvent], translation: &[CmarkEvent]) -> Vec<AlignAction> {
    // equal is good, align operation is not good
    let strategy = seal::pair::NeedlemanWunsch::new(1, -1, -1, 0);
    let set: seal::pair::AlignmentSet<seal::pair::InMemoryAlignmentMatrix> =
        seal::pair::AlignmentSet::new(translation.len(), source.len(), strategy, |x, y| {
            translation[x] == source[y]
        })
        .unwrap();
    let global_alignment = set.global_alignment();
    global_alignment
        .steps()
        .map(|step| {
            // x is valid in source and y is valid in target
            match step {
                // this element only exists in the source (was deleted) and not in translation
                seal::pair::Step::Delete { x } => {
                    let translation_element = translation.get(x).unwrap().clone();
                    AlignAction::Translation(translation_element)
                }
                // both sides are equal, pick from the source
                seal::pair::Step::Align { x, y } => {
                    let translation_element = translation.get(x).unwrap().clone();
                    let source_element = source.get(y).unwrap().clone();
                    if translation_element == source_element {
                        AlignAction::Both(translation_element)
                    } else {
                        AlignAction::Different(translation_element, source_element)
                    }
                }
                // this element only exists in the translation (was inserted) and not in source
                seal::pair::Step::Insert { y } => {
                    let source_element = source.get(y).unwrap().clone();
                    AlignAction::Source(source_element)
                }
            }
        })
        .collect()
}

/// diff the structure of to content-less CmarkEvent streams with the specified algorithm
pub fn diff_structure(
    source: &[CmarkEvent],
    translated: &[CmarkEvent],
    algorithm: &DiffAlgorithm,
) -> Vec<AlignAction> {
    match algorithm {
        DiffAlgorithm::Lcs => diff_structure_lcs(source, translated),
        DiffAlgorithm::NeedlemanWunsch => diff_structure_seal(source, translated),
    }
}
