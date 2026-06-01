//! Task tool renderer — the sub-agent dispatcher. Renders scalar args as a kv
//! list, the `prompt` argument as markdown, and a string result as markdown
//! (falling back to JSON). Mirrors the web `TaskArgs`.
//!
//! Source for (shared `frontend/llr/`):
//! - `Task tool renders prompt as markdown`

use serde_json::Value;

use super::content::parse_tool_call_result;
use super::BodyCtx;
use crate::tui::format::pretty_json;
use crate::tui::scenarios::spans::attrs::parse_tool_call_arguments;

/// Render the task args + result section.
pub(crate) fn render(ctx: &mut BodyCtx, attrs: &Value) {
    let args = parse_tool_call_arguments(attrs).filter(|v| !v.is_null());
    let result = parse_tool_call_result(attrs);

    if args.is_none() && result.is_none() {
        ctx.no_content();
        return;
    }

    if let Some(args) = &args {
        ctx.sublabel("arguments");
        match args.as_object() {
            Some(obj) => {
                let mut prompt: Option<String> = None;
                for (k, v) in obj {
                    if k == "prompt" {
                        if let Value::String(s) = v {
                            prompt = Some(s.clone());
                            continue;
                        }
                    }
                    let val = match v {
                        Value::String(s) => s.clone(),
                        other => pretty_json(other),
                    };
                    ctx.kv_row(k, &val);
                }
                if let Some(p) = &prompt {
                    ctx.sublabel("prompt");
                    ctx.markdown("task.prompt", p);
                }
            }
            None => ctx.json_text("task.args", args),
        }
    }

    if let Some(result) = &result {
        ctx.gap();
        ctx.sublabel("result");
        match result {
            Value::String(s) => ctx.markdown("task.result", s),
            other => ctx.json_text("task.result", other),
        }
    }
}
