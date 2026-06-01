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
