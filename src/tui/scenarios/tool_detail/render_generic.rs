//! Generic args/result renderer — the fallback body used by tools without a
//! specialized renderer, and by every external (MCP) tool span.
//!
//! Splits object args into "code-ish" string fields (those containing a
//! newline) rendered as their own searchable blocks, and the remaining
//! structured args rendered as one pretty-printed JSON block. Mirrors the web
//! `GenericArgs`.
//!
//! Source for (shared `frontend/llr/`):
//! - `Generic tool renders args splitting code-ish strings`
//! - `External tool detail uses generic args renderer`

use serde_json::Value;

use super::content::parse_tool_call_result;
use super::BodyCtx;
use crate::tui::scenarios::spans::attrs::parse_tool_call_arguments;

/// Render the generic args + result section.
pub(crate) fn render(ctx: &mut BodyCtx, attrs: &Value) {
    let args = parse_tool_call_arguments(attrs).filter(|v| !v.is_null());
    let result = parse_tool_call_result(attrs);

    if args.is_none() && result.is_none() {
        ctx.no_content();
        return;
    }

    if let Some(args) = &args {
        ctx.sublabel("arguments");
        render_args(ctx, args);
    }

    if let Some(result) = &result {
        ctx.gap();
        ctx.sublabel("result");
        match result {
            Value::String(s) => ctx.search_block("generic.result", s, None, None),
            other => ctx.json_text("generic.result", other),
        }
    }
}

/// Render the arguments value, splitting code-ish string fields out of the
/// object form. Shared by the generic and external renderers.
pub(crate) fn render_args(ctx: &mut BodyCtx, args: &Value) {
    match args.as_object() {
        Some(obj) => {
            let mut code_fields: Vec<(String, String)> = Vec::new();
            let mut rest = serde_json::Map::new();
            for (k, v) in obj {
                match v {
                    Value::String(s) if s.contains('\n') => {
                        code_fields.push((k.clone(), s.clone()));
                    }
                    _ => {
                        rest.insert(k.clone(), v.clone());
                    }
                }
            }
            for (k, v) in &code_fields {
                ctx.sublabel(k);
                ctx.search_block(&format!("generic.code.{k}"), v, None, None);
            }
            if !rest.is_empty() {
                ctx.json_text("generic.rest", &Value::Object(rest));
            } else if code_fields.is_empty() {
                ctx.json_text("generic.args", args);
            }
        }
        None => ctx.json_text("generic.args", args),
    }
}
