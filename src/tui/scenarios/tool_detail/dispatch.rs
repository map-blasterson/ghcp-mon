//! Tool-detail renderer dispatch — decides which specialized body renderer a
//! resolved [`SpanDetail`] routes to. Pure decision logic (no rendering).
//!
//! Source for (shared `frontend/llr/`):
//! - `Tool detail requires tool call projection`
//! - `Tool detail prefers native tool call over external`
//! - `Edit tool renders old new with syntax highlight`
//! - `View tool splits line numbers into gutter`
//! - `Task tool renders prompt as markdown`
//! - `Read agent tool renders result as markdown`
//! - `Generic tool renders args splitting code-ish strings`
//! - `External tool detail uses generic args renderer`

use crate::tui::model::SpanDetail;
use crate::tui::vendor::copilot::{tool_name_mapping, ToolKind};

/// Empty-state reasons that short-circuit rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyKind {
    /// Neither `tool_call` nor `external_tool_call` projection present.
    NotATool,
}

/// Which specialized renderer the span's args/result section uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererKind {
    Edit,
    View,
    Task,
    ReadAgent,
    Generic,
    /// External-origin (MCP) tool span: header + GenericArgs.
    External,
}

/// The top-level routing decision for a resolved span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Empty(EmptyKind),
    /// Native `tool_call` body with the chosen args renderer.
    Native(RendererKind),
    /// External `external_tool_call` body.
    External,
}

/// Decide the route for a resolved [`SpanDetail`]. Native `tool_call` is
/// preferred over `external_tool_call` per
/// `Tool detail prefers native tool call over external`.
pub fn route(detail: &SpanDetail) -> Route {
    let proj = &detail.projection;
    match (&proj.tool_call, &proj.external_tool_call) {
        (None, None) => Route::Empty(EmptyKind::NotATool),
        (Some(tc), _) => Route::Native(native_renderer(tc.tool_name.as_deref())),
        (None, Some(_)) => Route::External,
    }
}

/// Map a native tool's `tool_name` to the args renderer. Edit/write/read map
/// via the Copilot kind table; `task` / `read_agent` match by exact name;
/// everything else is Generic.
pub fn native_renderer(tool_name: Option<&str>) -> RendererKind {
    if let Some(name) = tool_name {
        match name {
            "task" => return RendererKind::Task,
            "read_agent" => return RendererKind::ReadAgent,
            _ => {}
        }
        if let Some(kind) = tool_name_mapping(name) {
            match kind {
                ToolKind::Edit | ToolKind::Write => return RendererKind::Edit,
                ToolKind::Read => return RendererKind::View,
                _ => {}
            }
        }
    }
    RendererKind::Generic
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_renderer_table() {
        assert_eq!(native_renderer(Some("edit")), RendererKind::Edit);
        assert_eq!(native_renderer(Some("create")), RendererKind::Edit); // write
        assert_eq!(native_renderer(Some("view")), RendererKind::View); // read
        assert_eq!(native_renderer(Some("task")), RendererKind::Task);
        assert_eq!(native_renderer(Some("read_agent")), RendererKind::ReadAgent);
        assert_eq!(native_renderer(Some("bash")), RendererKind::Generic); // shell → generic
        assert_eq!(native_renderer(Some("mystery_mcp")), RendererKind::Generic);
        assert_eq!(native_renderer(None), RendererKind::Generic);
    }
}
