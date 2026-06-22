//! Shared helper for replacing a YAML section in-place, preserving comments
//! and formatting of all unrelated content.
//!
//! Both [`user_settings`](super::user_settings) and
//! [`program_settings`](super::program_settings) need to update a single
//! root-level key without round-tripping the entire file through serde_yaml
//! (which strips all comments). This module provides that common logic.

use std::path::Path;

/// Replace the root-level `section_key:` YAML block in `path` with new body
/// content, or append the block if the key is not yet present.
///
/// `section_key` is the root-level YAML key (e.g. `"user_settings"`).
/// `yaml_lines` is the serialized YAML body **without** the section key line
/// (e.g. `"last_endpoint: openrouter\nlast_model: ...\n"`). The helper
/// prepends `section_key:\n` and indents each body line by two spaces.
///
/// All lines outside the replaced block - including comments, blank lines,
/// and other YAML keys - are preserved verbatim.
pub(crate) fn write_section_value(path: &Path, section_key: &str, yaml_lines: &str) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let new_section = build_new_section(section_key, yaml_lines);
    let pattern = format!("{}:", section_key);

    if let Some(start) = find_section_start(&content, &pattern) {
        let end = find_section_end(&content, start);
        let before = &content[..start];
        let after = &content[end..];
        let _ = std::fs::write(path, format!("{}{}{}", before, new_section, after));
    } else {
        // Section not found - append a blank line then the new block.
        let trimmed = content.trim_end();
        let _ = std::fs::write(path, format!("{}\n\n{}", trimmed, new_section));
    }
}

/// Build the replacement text for a section: `<key>:\n  <indented body>`.
fn build_new_section(key: &str, body: &str) -> String {
    let mut out = format!("{}:", key);
    for line in body.lines() {
        out.push('\n');
        out.push_str("  ");
        out.push_str(line);
    }
    out.push('\n');
    out
}

/// Find the byte offset of the line containing `pattern` (root-level only).
///
/// Searches for `pattern` at the start of a line with no leading whitespace,
/// i.e. root-level keys only. Returns `None` when the key is absent.
fn find_section_start(content: &str, pattern: &str) -> Option<usize> {
    let mut offset = 0;
    for line in content.lines() {
        if line.starts_with(pattern) {
            return Some(offset);
        }
        // +1 for the newline character consumed by .lines()
        offset += line.len() + 1;
    }
    None
}

/// Find the byte offset of the first line *after* the section body.
///
/// The section body starts on the line after `start` (the section key line).
/// Only indented lines (leading space or tab) are consumed as part of the
/// body. The first non-indented line - blank, comment, or another root-level
/// key - terminates the block. Returns the byte offset of that terminating
/// line, or `content.len()` if the body runs to EOF.
fn find_section_end(content: &str, start: usize) -> usize {
    let after_start = &content[start..];

    // Skip the first line (the section key itself).
    let newline_pos = match after_start.find('\n') {
        Some(p) => start + p + 1,
        None => return content.len(),
    };

    // Scan subsequent lines; only indented lines are part of the body.
    let remaining = &content[newline_pos..];
    let mut offset = 0;
    for line in remaining.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            offset += line.len() + 1;
        } else {
            // Blank, comment, or root-level key - block ends here.
            break;
        }
    }
    newline_pos + offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_temp(content: &str) -> (PathBuf, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("application.yaml");
        std::fs::write(&path, content).expect("write");
        (path, dir)
    }

    #[test]
    fn replaces_existing_section_in_place() {
        let initial = "\
endpoints: []
default_endpoint: openrouter

# ── User settings ────────────────────────────────────
user_settings:
  last_endpoint: old
  last_model: old

# ── Footer ───────────────────────────────────────────
";
        let (path, _dir) = write_temp(initial);

        write_section_value(
            &path,
            "user_settings",
            "last_endpoint: new\nlast_model: new\n",
        );

        let result = std::fs::read_to_string(&path).expect("read");
        assert!(result.contains("last_endpoint: new"));
        assert!(result.contains("last_model: new"));
        assert!(
            result.contains("# ── User settings"),
            "header comment lost:\n{}",
            result
        );
        assert!(
            result.contains("# ── Footer"),
            "footer comment lost:\n{}",
            result
        );
        assert!(result.contains("endpoints: []"), "endpoints lost");
    }

    #[test]
    fn appends_when_section_missing() {
        let initial = "\
# Only endpoints
endpoints: []
default_endpoint: openrouter
";
        let (path, _dir) = write_temp(initial);

        write_section_value(&path, "user_settings", "last_endpoint: openrouter\n");

        let result = std::fs::read_to_string(&path).expect("read");
        assert!(result.contains("# Only endpoints"), "header lost");
        assert!(result.contains("user_settings:\n  last_endpoint: openrouter"));
    }

    #[test]
    fn preserves_comments_between_sections() {
        let initial = "\
endpoints: []

# ── A comment between sections ───────────────────────
user_settings:
  last_endpoint: old
";
        let (path, _dir) = write_temp(initial);

        write_section_value(&path, "user_settings", "last_endpoint: new\n");

        let result = std::fs::read_to_string(&path).expect("read");
        assert!(
            result.contains("# ── A comment between sections"),
            "inter-section comment lost:\n{}",
            result
        );
        assert!(result.contains("last_endpoint: new"));
    }

    #[test]
    fn final_newline_handling() {
        let initial = "endpoints: []\n";
        let (path, _dir) = write_temp(initial);

        write_section_value(&path, "user_settings", "last_endpoint: x\n");
        let result = std::fs::read_to_string(&path).expect("read");
        assert!(result.ends_with('\n'), "must end with newline");
        assert!(result.contains("user_settings:"));
    }

    #[test]
    fn section_line_with_leading_spaces_is_not_root() {
        // Only root-level (column-0) keys are matched.
        let initial = "\
outer:
  user_settings:
    last_endpoint: old
root_key: val
";
        let (path, _dir) = write_temp(initial);

        write_section_value(&path, "user_settings", "last_endpoint: new\n");

        let result = std::fs::read_to_string(&path).expect("read");
        // The nested key under `outer:` should not be touched.
        assert!(
            result.contains("  user_settings:\n    last_endpoint: old"),
            "nested key was incorrectly replaced:\n{}",
            result
        );
        // The new root-level section should be appended.
        assert!(result.contains("user_settings:\n  last_endpoint: new"));
    }

    #[test]
    fn handles_complex_indented_body() {
        let initial = "\
# Some config
endpoints: []

user_settings:
  last_endpoint: openrouter
  last_model: \"model/v1\"
  last_reasoning_effort: high
";
        let (path, _dir) = write_temp(initial);

        write_section_value(
            &path,
            "user_settings",
            "last_endpoint: copilot\nlast_model: \"claude-4\"\nlast_reasoning_effort: medium\n",
        );

        let result = std::fs::read_to_string(&path).expect("read");
        assert!(result.contains("# Some config"));
        assert!(result.contains("endpoints: []"));
        assert!(result.contains("last_endpoint: copilot"));
        assert!(result.contains("last_model: \"claude-4\""));
        assert!(result.contains("last_reasoning_effort: medium"));
        // Old values gone
        assert!(!result.contains("deepseek/deepseek-v4-flash"));
        assert!(!result.contains("high"));
    }

    #[test]
    fn blank_lines_and_comments_after_section_are_preserved() {
        let initial = "\
endpoints: []

user_settings:
  last_endpoint: old
  last_model: old

# ── Footer ───────────────────────────────────────────
# Also this comment after a blank line
";
        let (path, _dir) = write_temp(initial);

        write_section_value(
            &path,
            "user_settings",
            "last_endpoint: new\nlast_model: new\n",
        );

        let result = std::fs::read_to_string(&path).expect("read");
        assert!(
            result.contains("# ── Footer"),
            "footer comment lost:\n{}",
            result
        );
        assert!(
            result.contains("# Also this comment after a blank line"),
            "second footer comment lost:\n{}",
            result
        );
        assert!(result.contains("last_endpoint: new"));
        assert!(result.contains("last_model: new"));
    }
}
