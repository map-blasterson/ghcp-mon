//! RawBrowser scenario — currently a Phase-0 placeholder renderer that
//! dumps the column's `config` so cross-column routing can be inspected
//! without a real renderer.
//!
//! Wraps [`super::render_placeholder`] in a [`Scenario`] impl so dispatch
//! goes through the same path as the migrated scenarios; eliminates the
//! "fall through to placeholder" arm in `App::draw_workspace`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::tui::app::DrawOutcome;
use crate::tui::workspace::{ColumnConfig, ScenarioType};

use super::scenario::{Ctx, KeyOutcome, Scenario};

#[derive(Debug, Default)]
pub struct RawBrowserScenario;

impl RawBrowserScenario {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Scenario for RawBrowserScenario {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn draw(
        &mut self,
        _ctx: &mut Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        config: &ColumnConfig,
        area: Rect,
        buf: &mut Buffer,
        _focused: bool,
        _outcome: &mut DrawOutcome,
    ) {
        super::render_placeholder(area, buf, ScenarioType::RawBrowser, config);
    }

    fn handle_key(
        &mut self,
        _ctx: &mut Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        _config: &ColumnConfig,
        _k: KeyEvent,
    ) -> KeyOutcome {
        KeyOutcome::pass()
    }

    fn keymap_entries(&self, _config: &ColumnConfig) -> Vec<(String, String)> {
        Vec::new()
    }
}
