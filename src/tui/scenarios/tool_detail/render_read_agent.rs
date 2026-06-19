//! Read-agent tool renderer — the sub-agent inspector. Renders all args as a
//! plain kv list and a string result as markdown (falling back to JSON).
//! Mirrors the web `ReadAgentArgs`.
//!
//! Source for (shared `frontend/llr/`):
//! - `Read agent tool renders result as markdown`

use serde_json::Value;

use super::content::parse_tool_call_result;
use super::BodyCtx;
use crate::tui::format::pretty_json;
use crate::tui::scenarios::spans::attrs::parse_tool_call_arguments;

/// Render the read_agent args + result section.
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
                for (k, v) in obj {
                    let val = match v {
                        Value::String(s) => s.clone(),
                        other => pretty_json(other),
                    };
                    ctx.kv_row(k, &val);
                }
            }
            None => ctx.json_text("read_agent.args", args),
        }
    }

    if let Some(result) = &result {
        ctx.gap();
        ctx.sublabel("result");
        match result {
            Value::String(s) => ctx.markdown("read_agent.result", s),
            other => ctx.json_text("read_agent.result", other),
        }
    }
}
