//! Animated rolling-dots indicator. 4-frame cycle (`   ` / `.  ` / `.. ` /
//! `...`) advanced once every [`FRAME_STEP_MS`] of wall-clock time
//! (≈ 250 ms).
//!
//! Implements `Placeholder ingestion state shown with rolling dots` (TUI
//! variant) and `TUI Spans rolling dots animation cadence`. The cadence is
//! derived from wall-clock so the event loop does not need a heartbeat
//! tick to drive it — the loop instead schedules a redraw at the next
//! [`FRAME_STEP_MS`] boundary whenever a spinner is on screen.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

/// Wall-clock cadence of one dot-frame step.
pub const FRAME_STEP_MS: u64 = 250;

/// Return the 3-character frame for a given wall-clock millisecond reading.
pub fn frame_at(now_ms: u64) -> &'static str {
    match (now_ms / FRAME_STEP_MS) % 4 {
        0 => "   ",
        1 => ".  ",
        2 => ".. ",
        _ => "...",
    }
}

pub struct RollingDots {
    pub now_ms: u64,
}

impl Widget for RollingDots {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let txt = frame_at(self.now_ms);
        let span = Span::styled(txt, Style::default().fg(Color::Yellow));
        buf.set_span(area.x, area.y, &span, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_four_frames() {
        assert_eq!(frame_at(0), "   ");
        assert_eq!(frame_at(FRAME_STEP_MS), ".  ");
        assert_eq!(frame_at(FRAME_STEP_MS * 2), ".. ");
        assert_eq!(frame_at(FRAME_STEP_MS * 3), "...");
        assert_eq!(frame_at(FRAME_STEP_MS * 4), "   ");
    }
}
