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

### Phase 3 additions (Tool Detail column)

Active when a `ToolDetail` column has focus. `Tab`/`Shift-Tab` cycle the column's focusable blocks (metadata panel, searchable bodies, JSON panels) before falling through to the global column-focus cycle.

| Key             | Layer      | Mode/Scope                        | Effect                                              |
| --------------- | ---------- | --------------------------------- | --------------------------------------------------- |
| `Tab`           | column     | block focus < last                | focus next block (else fall through, reset to first)|
| `Shift-Tab`     | column     | block focus > 0                   | focus previous block (else fall through)            |
| `↑` / `↓`       | column     | any                               | scroll body up / down one line                      |
| `Home` / `End`  | column     | any                               | scroll to top / bottom                              |
| `Space`         | column     | focused metadata / JSON panel     | toggle panel open / closed (`▸`/`▾`)                 |
| `/`             | widget     | focused searchable / open JSON    | activate per-block search (then text-input layer)   |
| `Enter`         | widget     | block search Active               | next match (Shift+Enter previous)                   |
| `Esc`           | widget     | block search Active               | exit search (no-op while external query drives it)  |

### Phase 4 additions (Chat Detail column)

Active when a `ChatDetail` column has focus.

| Key                | Layer  | Mode/Scope                                | Effect                                                |
| ------------------ | ------ | ----------------------------------------- | ----------------------------------------------------- |
| `↑` / `↓`          | column | tree focused                              | move row cursor                                       |
| `←`                | column | tree focused                              | collapse focused node                                 |
| `→`                | column | tree focused                              | expand focused node                                   |
| `Space`            | column | tree focused, primitive key row           | toggle the focused primitive's expand state           |
| `Space`            | column | tree focused, normal row                  | toggle node expand                                    |
| `m`                | column | always                                    | toggle `chat_mode` DELTA ↔ FULL                       |
| `Tab` / `Shift-Tab`| column | within ChatDetail                         | cycle focus across body sub-blocks                    |
| `Home` / `End`     | column | tree focused                              | jump to top / bottom                                  |

(Widget-layer keys for SearchableTextBlock — `/`, `Enter`, `Shift+Enter`, `Esc`, printable, Backspace — remain unchanged from Phase 2.5.)

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
- [[TUI Tool detail bottom-up layout in cells]]
- [[TUI Tool detail key-dispatch precedence within column]]
- [[TUI Tool detail metadata panel default closed]]
- [[TUI Tool detail empty state verbatim copy]]
- [[TUI CodeBlock syntect highlight rendering]]
- [[TUI Markdown to lines via pulldown-cmark]]
- [[TUI Udiff classify line precedence]]
- [[TUI JsonView collapsed default closed]]
- [[TUI Chat detail layout in cells]]
- [[TUI Chat detail summary bar paints via Buffer cell_mut]]
- [[TUI Chat detail key-cursor indicator on focused key row]]
- [[TUI Chat detail tool-call arrow gutter]]
- [[TUI Chat detail node id is slash-delimited path]]
- [[TUI Chat detail focus precedence within column]]
- [[TUI Chat detail mode chip in header]]
- [[TUI Chat detail search-expanded set tracks restoration]]
