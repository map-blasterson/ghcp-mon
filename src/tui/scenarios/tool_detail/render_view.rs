//! View/read tool renderer — path + extra args, and a result body that strips
//! the opencode `<path>/<type>/<content>` XML envelope and the `N. `/`N: `
//! line-number prefixes into a dim left gutter, syntect-highlighting the
//! remaining source by the file extension. Mirrors the web `ViewArgs`.
//!
//! Source for (shared `frontend/llr/`):
//! - `View tool splits line numbers into gutter`

use serde_json::Value;

use super::content::parse_tool_call_result;
use super::BodyCtx;
use crate::tui::scenarios::spans::attrs::parse_tool_call_arguments;
use crate::tui::widgets::code_block::syntect_byte_styles;
use crate::tui::widgets::lang_from_path::lang_from_path;

/// Strip an opencode `<path>…</path><type>…</type><content>…</content>` wrapper,
/// returning the inner content when the whole body matches, else the input
/// unchanged. Implemented without the `regex` crate.
pub fn strip_xml_wrapper(body: &str) -> &str {
    let t = body.trim();
    let Some(rest) = t.strip_prefix("<path>") else {
        return body;
    };
    let Some(pe) = rest.find("</path>") else {
        return body;
    };
    let rest = rest[pe + "</path>".len()..].trim_start();
    let Some(rest) = rest.strip_prefix("<type>") else {
        return body;
    };
    let Some(te) = rest.find("</type>") else {
        return body;
    };
    let rest = rest[te + "</type>".len()..].trim_start();
    let Some(rest) = rest.strip_prefix("<content>") else {
        return body;
    };
    let Some(ce) = rest.rfind("</content>") else {
        return body;
    };
    rest[..ce].trim_matches(['\n', '\r'])
}

/// Split `N. ` / `N: ` line-number prefixes off each line. Returns the parsed
/// numbers (per line; `None` when a line had no prefix) and the body with the
/// prefixes removed. If **no** line carried a prefix, returns `(empty, body)`
/// so callers can skip the gutter. Equivalent to the web `/^(\d+)[.:]\s(.*)$/`
/// per-line strip; implemented without the `regex` crate.
pub fn strip_line_numbers(body: &str) -> (Vec<Option<u64>>, String) {
    let mut nums: Vec<Option<u64>> = Vec::new();
    let mut stripped: Vec<String> = Vec::new();
    let mut any = false;
    for line in body.split('\n') {
        match parse_line_prefix(line) {
            Some((n, rest)) => {
                any = true;
                nums.push(Some(n));
                stripped.push(rest.to_string());
            }
            None => {
                nums.push(None);
                stripped.push(line.to_string());
            }
        }
    }
    if any {
        (nums, stripped.join("\n"))
    } else {
        (Vec::new(), body.to_string())
    }
}

/// Match `^(\d+)[.:]\s(.*)$` on a single line.
fn parse_line_prefix(line: &str) -> Option<(u64, &str)> {
    let digits_end = line.find(|c: char| !c.is_ascii_digit())?;
    if digits_end == 0 {
        return None;
    }
    let bytes = line.as_bytes();
    let sep = bytes[digits_end];
    if sep != b'.' && sep != b':' {
        return None;
    }
    // Require exactly one whitespace separator after the `.`/`:`.
    let after = &line[digits_end + 1..];
    let mut chars = after.char_indices();
    let (_, ws) = chars.next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let rest = &after[ws.len_utf8()..];
    let n: u64 = line[..digits_end].parse().ok()?;
    Some((n, rest))
}

/// Resolve the file body from a result value (string directly, or `output`).
fn resolve_body(result: &Value) -> Option<String> {
    match result {
        Value::String(s) => Some(s.clone()),
        other => other
            .as_object()
            .and_then(|o| o.get("output"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

/// Render the view/read args + result section.
pub(crate) fn render(ctx: &mut BodyCtx, attrs: &Value) {
    let args = parse_tool_call_arguments(attrs).filter(|v| !v.is_null());
    let result = parse_tool_call_result(attrs);
    let args_obj = args.as_ref().and_then(|v| v.as_object()).cloned();

    if args_obj.is_none() && result.is_none() {
        ctx.no_content();
        return;
    }

    let mut path: Option<String> = None;
    if let Some(obj) = &args_obj {
        ctx.sublabel("arguments");
        path = obj
            .get("path")
            .or_else(|| obj.get("filePath"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if let Some(p) = &path {
            ctx.kv_row("path", p);
        }
        let mut extra = serde_json::Map::new();
        for (k, v) in obj {
            if k == "path" || k == "filePath" {
                continue;
            }
            extra.insert(k.clone(), v.clone());
        }
        if !extra.is_empty() {
            ctx.sublabel("other");
            ctx.json_text("view.other", &Value::Object(extra));
        }
    }

    if let Some(result) = &result {
        ctx.gap();
        ctx.sublabel("result");
        let lang = path.as_deref().and_then(lang_from_path);
        match resolve_body(result) {
            Some(raw) => {
                let unwrapped = strip_xml_wrapper(&raw).to_string();
                let (nums, body) = strip_line_numbers(&unwrapped);
                let styles = syntect_byte_styles(&body, lang);
                if nums.is_empty() {
                    ctx.search_block("view.result", &body, styles, None);
                } else {
                    ctx.search_block("view.result", &body, styles, Some(&nums));
                }
            }
            None => ctx.json_text("view.result", result),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_dot_prefixes_into_gutter() {
        let (nums, body) = strip_line_numbers("1. fn main() {\n2. }\n");
        assert_eq!(nums, vec![Some(1), Some(2), None]);
        assert_eq!(body, "fn main() {\n}\n");
    }

    #[test]
    fn strips_colon_prefixes() {
        let (nums, body) = strip_line_numbers("10: a\n11: b");
        assert_eq!(nums, vec![Some(10), Some(11)]);
        assert_eq!(body, "a\nb");
    }

    #[test]
    fn no_prefixes_returns_empty_gutter() {
        let (nums, body) = strip_line_numbers("plain text\nno numbers");
        assert!(nums.is_empty());
        assert_eq!(body, "plain text\nno numbers");
    }

    #[test]
    fn mixed_lines_keep_alignment() {
        let (nums, body) = strip_line_numbers("1. code\nbare\n3. more");
        assert_eq!(nums, vec![Some(1), None, Some(3)]);
        assert_eq!(body, "code\nbare\nmore");
    }

    #[test]
    fn unwraps_xml_envelope() {
        let b = "<path>a.rs</path>\n<type>file</type>\n<content>\nfn x() {}\n</content>";
        assert_eq!(strip_xml_wrapper(b), "fn x() {}");
    }

    #[test]
    fn non_wrapped_body_unchanged() {
        assert_eq!(strip_xml_wrapper("just text"), "just text");
    }
}
