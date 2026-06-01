---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/keymap
---
The cumulative key table registered by the TUI. Each phase appends rows; this is the Phase 0 baseline.

| Mode    | Key            | Effect                                 |
| ------- | -------------- | -------------------------------------- |
| Global  | `q`            | Quit                                   |
| Global  | `Ctrl-C`       | Quit                                   |
| Global  | `Tab`          | Focus next column                      |
| Global  | `Shift-Tab`    | Focus previous column                  |
| Global  | `?`            | Toggle log overlay                     |
| Global  | `M`            | Toggle mouse capture                   |
| Global  | `a`            | Append a column (cycle scenario type)  |
| Global  | `x`            | Remove the focused column              |
| Modal   | `?` / `Esc`    | Close log overlay                      |

### Phase 1 additions

| Mode    | Key            | Effect                                                |
| ------- | -------------- | ----------------------------------------------------- |
| Column  | `↑` / `↓`      | Spans tree / LiveSessions: move row cursor            |
| Column  | `←`            | Spans tree: collapse focused row                      |
| Column  | `→`            | Spans tree: expand focused row                        |
| Column  | `Space`        | Spans tree: toggle focused row collapse/expand        |
| Column  | `Home` / `End` | Spans / LiveSessions: jump to top / bottom            |
| Column  | `+`            | Spans header: expand all                              |
| Column  | `-`            | Spans header: collapse all                            |
| Column  | `f`            | Spans: toggle follow-mode                             |
| Column  | `/`            | Spans: focus search input (enters text-input mode)    |
| Column  | `Enter`        | Spans → propagate selection; Sessions → pick session  |
| Column  | `d` / `Delete` | LiveSessions: open delete-session confirm modal       |
| Column  | `s`            | Spans: open session selector popover                  |
| Column  | `k`            | Spans: open kind filter popover                       |
| Modal   | `y` / `n`      | Confirm modal: confirm / cancel                       |
| Modal   | `Enter`        | Confirm modal: dismiss with current selection         |
| Modal   | `Esc`          | Confirm modal: dismiss as No; search input: exit      |
| Input   | printable      | Search input: append char                             |
| Input   | `←` / `→`      | Search input: move cursor                             |
| Input   | `Home` / `End` | Search input: jump cursor to start / end              |
| Input   | `Backspace`    | Search input: delete char left                        |
| Input   | `Delete`       | Search input: delete char right                       |

### Phase 2 additions (Context Growth Widget)

| Mode    | Key                | Effect                                              |
| ------- | ------------------ | --------------------------------------------------- |
| Global  | `c`                | Toggle Context Growth Widget visibility             |
| Global  | `Alt+↑` / `Alt+↓`  | Grow / shrink widget by 1 row (when visible)        |
| Global  | `Alt+Shift+↑/↓`    | Grow / shrink widget by 5 rows (when visible)       |
| Global  | `Tab` / `Shift-Tab`| Focus cycle includes the widget slot when visible   |
| Widget  | `←` / `→`          | Move bar cursor (publishes cross-column hover)       |
| Widget  | `Enter`            | Select cursor bar → routes to Spans column selection |
| Widget  | `Esc`              | Release widget focus back to columns                 |

### Phase 2.5 additions (SearchableTextBlock widget)

Active only when a `SearchableTextBlock` has focus (the **widget** precedence layer; its `Active`-phase editing is the **text-input** layer).

| Key             | Layer      | Mode/Scope                   | Effect                                            |
| --------------- | ---------- | ---------------------------- | ------------------------------------------------- |
| `/`             | widget     | Icon phase                   | activate search input (Idle/Icon → Active)        |
| `Enter`         | widget     | Active phase                 | next match (wraps)                                |
| `Shift+Enter`   | widget     | Active phase                 | previous match (wraps)                            |
| `Esc`           | widget     | Active phase                 | exit search (no-op when `external_query` is set)  |
| printable chars | text-input | Active phase + input focused | append to query                                   |
| `Backspace`     | text-input | Active phase + input focused | delete last query char                            |

## Derived LLRs
- [[TUI top bar appends column via 'a' keystroke]]
- [[TUI top bar removes focused column via 'x' keystroke]]
- [[TUI Spans search input edit semantics]]
- [[TUI Spans tree row layout in cells]]
- [[TUI Spans focused row publishes hovered chat ancestor]]
- [[TUI Live sessions row layout in cells]]
- [[TUI Confirm modal default no]]
- [[TUI Spans header two-row layout in terminal cells]]
- [[TUI Spans rolling dots animation cadence]]
- [[TUI Reveal schedule advances on every tick]]
- [[TUI Spans bottom detail pane layout]]
- [[TUI Spans traces list mode]]
- [[TUI Context widget keyboard bar cursor navigation]]
- [[TUI Context widget Alt arrow height adjustment]]
- [[TUI Context widget collapsed single-row bar]]
- [[TUI Context widget participates in Tab focus cycle]]
