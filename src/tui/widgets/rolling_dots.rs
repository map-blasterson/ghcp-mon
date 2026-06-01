//! Animated rolling-dots indicator. 4-frame cycle (`   ` / `.  ` / `.. ` /
//! `...`) advanced once every `FRAMES_PER_STEP` ticks (≈ 250 ms at 16 ms
//! ticks).
//!
//! Implements `Placeholder ingestion state shown with rolling dots` (TUI
//! variant) and `TUI Spans rolling dots animation cadence`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

pub const FRAMES_PER_STEP: u64 = 4;

/// Return the 3-character frame for a given tick counter.
pub fn frame(tick: u64) -> &'static str {
    match (tick / FRAMES_PER_STEP) % 4 {
        0 => "   ",
        1 => ".  ",
        2 => ".. ",
        _ => "...",
    }
}

pub struct RollingDots {
    pub tick: u64,
}

impl Widget for RollingDots {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let txt = frame(self.tick);
        let span = Span::styled(txt, Style::default().fg(Color::Yellow));
        buf.set_span(area.x, area.y, &span, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_four_frames() {
        assert_eq!(frame(0), "   ");
        assert_eq!(frame(FRAMES_PER_STEP), ".  ");
        assert_eq!(frame(FRAMES_PER_STEP * 2), ".. ");
        assert_eq!(frame(FRAMES_PER_STEP * 3), "...");
        assert_eq!(frame(FRAMES_PER_STEP * 4), "   ");
    }
}
