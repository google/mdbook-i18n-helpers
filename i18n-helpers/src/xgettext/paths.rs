const POT_EXTENSION: &str = "pot";

use std::collections::HashMap;
use std::path::PathBuf;

/// Mimics the `push` and `pop` functionalities of [`PathBuf`] with,
/// respectively, the [`push`] and [`pop`] methods, so that:
///
/// 1. The path is never extended past `max_depth` depth.
/// 2. Whenever [`push`] is invoked, the base directory/file of the path is
///    potentially altered to make the path unique among the ones produced thus
///    far.
/// 3. All strings passed to [`push`] are normalized by retaining only
///    non-alphabetic characters and hyphens.
///
/// The disambiguation is deterministic (i.e., the same sequence of operations
/// is guaranteed to always produce the same paths) and not based on
/// randomization, therefore its stability doesn't depend on
/// the stability of external libraries.
///
/// The [`get`] method returns the current path with a .pot extension.
///
/// The spacial case of `depth` equal to 0 is treated differently by returning
/// the [`DEFAULT_DEPTH_0`](UniquePathBuilder::DEFAULT_DEPTH_0).pot filename.
///
/// [`push`]: UniquePathBuilder::push
/// [`pop`]: UniquePathBuilder::pop
/// [`get`]: UniquePathBuilder::get
/// ```
pub struct UniquePathBuilder {
    current_path: PathBuf,
    current_depth: usize,
    max_depth: usize,
    frequency_map: HashMap<PathBuf, usize>,
}

impl UniquePathBuilder {
    const DEFAULT_DEPTH_0: &str = "messages";

    pub fn new(max_depth: usize) -> Self {
        Self {
            current_path: PathBuf::new(),
            current_depth: 0,
            max_depth,
            frequency_map: HashMap::new(),
        }
    }

    fn format_path_with_counter(&self) -> PathBuf {
        match self.frequency_map.get(&self.current_path) {
            None | Some(&0) => self.current_path.clone(),
            Some(&cnt) => {
                let mut file_name = self.current_path.file_name().unwrap().to_os_string();
                file_name.push(format!("-{}", cnt));
                self.current_path.with_file_name(file_name)
            }
        }
    }

    pub fn get(&self) -> PathBuf {
        if self.max_depth == 0 || self.current_depth == 0 {
            PathBuf::from(Self::DEFAULT_DEPTH_0).with_extension(POT_EXTENSION)
        } else {
            self.format_path_with_counter()
                .with_extension(POT_EXTENSION)
        }
    }

    pub fn push<T: AsRef<str>>(&mut self, s: T) {
        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            return; // Exceeded max depth, do nothing.
        }
        // Extend current path with new component.
        self.current_path = self.format_path_with_counter();
        self.current_path.push(slug(s.as_ref()));
        // Update frequency of new path.
        self.frequency_map
            .entry(self.current_path.clone())
            .and_modify(|cnt| *cnt += 1)
            .or_insert(0);
    }

    pub fn pop(&mut self) {
        if self.current_depth == 0 {
            return; // Maybe panic instead?
        }
        if self.current_depth <= self.max_depth {
            self.current_path.pop();
        }
        self.current_depth -= 1;
    }
}

// Trims a string slice to only contain alphanumeric characters and dashes.
// If the resulting string is empty, returns `"null"` instead.
fn slug(title: &str) -> String {
    // Specially handle "C++" to format it as "cpp" instead of "c".
    let title = title.to_lowercase().replace("c++", "cpp");
    let title = title
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|&ch| ch == '-' || ch.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if title.is_empty() {
        String::from("null")
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::UniquePathBuilder;

    fn get_str(pb: &UniquePathBuilder) -> String {
        pb.get().to_str().unwrap().into()
    }

    #[test]
    fn test_basic_usage() {
        let mut unique_path_builder = UniquePathBuilder::new(2);

        unique_path_builder.push("foo"); // Pushes "foo"
        unique_path_builder.push("bar"); // Pushes "bar"

        assert_eq!(get_str(&unique_path_builder), "foo/bar.pot");

        unique_path_builder.push("baz"); // No-op for the user

        assert_eq!(get_str(&unique_path_builder), "foo/bar.pot");

        unique_path_builder.pop(); // No-op
        unique_path_builder.pop(); // Pops "bar"
        unique_path_builder.pop(); // Pops "foo"
        unique_path_builder.push("foo"); // Pushes "foo_1"
        unique_path_builder.push("bar"); // Pushes "bar"
        unique_path_builder.pop(); // Pops "bar"
        unique_path_builder.push("bar"); // Pushes "bar_1"

        assert_eq!(get_str(&unique_path_builder), "foo-1/bar-1.pot");
    }
}
