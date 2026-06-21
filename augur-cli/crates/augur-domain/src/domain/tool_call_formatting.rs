//! Shared tool-call formatting used by main and agent-feed panels.

use crate::domain::string_newtypes::{OutputText, StringNewtype, ToolName};

const FILE_CREATE_PREVIEW_LINE_LIMIT: usize = 3;
const FILE_CREATE_PREVIEW_CHAR_LIMIT: usize = 160;

/// Format a tool-call summary line (or multiline summary) from a tool name and JSON args.
pub fn format_tool_call_line(name: ToolName, args: &serde_json::Value) -> OutputText {
    let rendered = if let Some(args_obj) = args.as_object() {
        match name.as_str() {
            "view" => format_view_call(args_obj),
            "bash" => format_bash_call(args_obj),
            "glob" => format_glob_call(args_obj),
            "grep" => format_grep_call(args_obj),
            "file_create" => format_file_create_call(args_obj),
            _ => format_default_call(name.as_str(), args_obj),
        }
    } else {
        format!("  \u{2192} {}: {}", name.as_str(), args)
    };
    OutputText::new(rendered)
}

fn format_view_call(args: &serde_json::Map<String, serde_json::Value>) -> String {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    if let Some(range) = args.get("view_range").and_then(|v| v.as_array()) {
        let range_str = range
            .iter()
            .filter_map(|v| v.as_i64())
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("  \u{2192} view: {}\n    [lines: {}]", path, range_str)
    } else {
        format!("  \u{2192} view: {}", path)
    }
}

fn format_bash_call(args: &serde_json::Map<String, serde_json::Value>) -> String {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("bash");
    format!("  \u{2192} {}\n    {}", description, command)
}

fn format_glob_call(args: &serde_json::Map<String, serde_json::Value>) -> String {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    format!("  \u{2192} glob: (pattern)\n    {}", pattern)
}

fn format_grep_call(args: &serde_json::Map<String, serde_json::Value>) -> String {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    format!("  \u{2192} grep: (pattern)\n    {}", pattern)
}

fn format_file_create_call(args: &serde_json::Map<String, serde_json::Value>) -> String {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let all_lines: Vec<&str> = if content.is_empty() {
        Vec::new()
    } else {
        content.split('\n').collect()
    };
    let shown_count = all_lines.len().min(FILE_CREATE_PREVIEW_LINE_LIMIT);
    let mut rendered = format!("  \u{2192} file_create: {}", path);
    if shown_count == 0 {
        rendered.push_str("\n    (empty content)");
        return rendered;
    }

    for line in all_lines.iter().take(FILE_CREATE_PREVIEW_LINE_LIMIT) {
        rendered.push_str("\n    ");
        rendered.push_str(&truncate_file_create_preview_line(line));
    }

    let omitted = all_lines.len().saturating_sub(shown_count);
    if omitted > 0 {
        rendered.push_str(&format!("\n    ... (+{} more lines)", omitted));
    }
    rendered
}

fn truncate_file_create_preview_line(line: &str) -> String {
    let mut chars = line.chars();
    let preview: String = chars
        .by_ref()
        .take(FILE_CREATE_PREVIEW_CHAR_LIMIT)
        .collect();
    if chars.next().is_some() {
        format!("{}...", preview)
    } else {
        preview
    }
}

fn format_default_call(name: &str, args: &serde_json::Map<String, serde_json::Value>) -> String {
    let args_summary = args
        .values()
        .find_map(|v| v.as_str())
        .unwrap_or("(args)")
        .to_owned();
    format!("  \u{2192} {}: {}", name, args_summary)
}
