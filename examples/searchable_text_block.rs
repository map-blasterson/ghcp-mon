//! Standalone demo for the [`SearchableTextBlock`] widget.
//!
//! ```bash
//! cargo run --example searchable_text_block
//! cargo run --example searchable_text_block -- --external-query foo
//! ```
//!
//! Keybinds:
//! - `/`           activate search (Idle/Icon → Active)
//! - characters    edit query
//! - `Enter`       next match (wraps)
//! - `Shift+Enter` previous match (wraps)
//! - `Esc`         exit search (no-op when `--external-query` is set)
//! - `q`           quit the example (when search is not active)

use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use ghcp_mon::tui::widgets::searchable_text_block::{
    SearchableTextBlock, SearchableTextBlockState,
};

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod \
tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor \
in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur \
sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est \
laborum. The word lorem appears several times: lorem, Lorem, LOREM — try searching for it.";

fn main() -> io::Result<()> {
    // Optional `--external-query <s>` to demo programmatic lifecycle suppression.
    let mut external_query: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--external-query" {
            external_query = args.next();
        }
    }

    let mut terminal = ratatui::init();
    let mut state = SearchableTextBlockState::default();
    let result = run(&mut terminal, &mut state, external_query.as_deref());
    ratatui::restore();
    result
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut SearchableTextBlockState,
    external_query: Option<&str>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let area = f.area();
            // Centre a fixed-size block in the terminal.
            let vchunks = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(16),
                Constraint::Fill(1),
            ])
            .split(area);
            let hchunks = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(60),
                Constraint::Fill(1),
            ])
            .split(vchunks[1]);
            let outer = hchunks[1];

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" SearchableTextBlock demo ")
                .style(Style::default().fg(Color::Gray));
            let inner = block.inner(outer);
            f.render_widget(block, outer);

            // Footer help line at the bottom of the terminal.
            let help = Line::from(
                "/ activate · type to edit · Enter/Shift+Enter cycle · Esc exit · q quit",
            )
            .style(Style::default().fg(Color::DarkGray));
            let footer = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
            f.render_widget(Paragraph::new(help), footer);

            let widget = SearchableTextBlock {
                text: LOREM,
                truncatable: false,
                open: true,
                truncate_rows: 0,
                external_query,
                focused: true,
            };
            widget.render(inner, f.buffer_mut(), state);
        })?;

        if !event::poll(Duration::from_millis(150))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            // The widget owns the key when active (or when `/` activates it).
            let consumed = SearchableTextBlock::handle_key(key, state, external_query);
            if consumed {
                continue;
            }
            // Only quit when the widget did not consume the key.
            if matches!(key.code, KeyCode::Char('q')) {
                return Ok(());
            }
        }
    }
}
