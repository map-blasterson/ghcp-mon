# components — Memory Bank (frontend)

## Purpose
Reusable React/TypeScript building blocks under `web/src/components/` that compose
the dashboard's workspace chrome, scenario column shell, and the cross-cutting
inspectors and formatters used by every scenario view. Components in this folder
are agnostic of any single scenario: they implement layout, content parsing,
syntax/highlight rendering, span inspection, and the persistent context-growth
chart. Higher-level scenario views in `web/src/scenarios/` consume these
primitives via the registry plumbed through `Column`.

## Key behaviors

### Workspace & Column layout

- `Workspace` measures its container's pixel width with `ResizeObserver` and
  distributes that width across columns proportionally to each column's `width`
  weight, after subtracting `4 px` per inter-column resizer
  (`Workspace lays out columns by weight with ResizeObserver`).
- Every column is clamped to at least `MIN_COL_PX = 120 px`; the deficit from
  pinned-to-minimum columns is redistributed to the unpinned ones, then widths
  are rounded to integer pixels and the rounding remainder is absorbed into the
  last unpinned column so the pixel sum equals the available width
  (`Workspace lays out columns by weight with ResizeObserver`,
  `Workspace minimum column width 120 px`).
- The `120 px` floor applies regardless of weight, except in the degenerate case
  where the available width is itself smaller than the sum of column minimums
  (`Workspace minimum column width 120 px`).
- Dragging an inter-column resizer is a strictly local rebalance: only the two
  adjacent columns change, their pixel sum stays equal to its pre-drag value,
  the left pixel width is clamped to `[MIN_COL_PX, total - MIN_COL_PX]`, and
  each new weight is `totalWeight * (newPx / totalPx)`
  (`Resizer drag rebalances two adjacent column weights`).
- When `useWorkspace().columns` is empty, `Workspace` renders the literal
  empty-state message `"no columns. add one from the top bar."` instead of the
  grid (`Empty workspace shows empty state`).
- `ColumnBody` switches on `column.scenarioType` and renders the corresponding
  scenario component: `live_sessions → LiveSessionsScenario`, `spans →
  SpansScenario`, `tool_detail → ToolDetailScenario`, `raw_browser →
  RawBrowserScenario`, `chat_detail → ChatDetailScenario`, `file_touches →
  FileTouchesScenario` — the only place that knows the full scenario set
  (`Column body dispatches by scenario type`).
- `ColumnHeader` renders an editable title input whose `onChange` calls
  `updateColumn(id, { title })`, plus three action buttons that call
  `moveColumn(id, -1)`, `moveColumn(id, 1)`, and `removeColumn(id)`. When its
  `children` prop (per-scenario filters) is empty it renders `"no filters"` as
  a muted subtitle (`Column header allows rename move remove`).

### Inspectors (Inspector, KindBadge)

- `SpanInspector` issues `useQuery({ queryKey: ["span", trace_id, span_id],
  queryFn: () => api.getSpan(trace_id, span_id) })`, renders `"loading…"` while
  pending, `"span not found"` on error or empty data, and otherwise the
  `SpanDetailView` of the response; the query key is shared with sibling
  scenarios so the cache hits across columns
  (`Span inspector fetches and renders detail`).
- `SpanDetailView` renders every projection sub-block present on
  `detail.projection` — `chat_turn`, `tool_call`, `agent_run`,
  `external_tool_call` — each inside an open `<details>` element whose body is
  a `JsonView` of the projection record; the projection section is omitted
  entirely when `projection` is empty
  (`Span detail view renders projection sub-blocks`).
- The `relations` section of `SpanDetailView` shows the parent as
  `"<parent.name> (<8-char span_id>)"` (or `—` when there is no parent) and
  lists every child `SpanRef` with its name, a `KindBadge`, and the 8-character
  span id (`Span detail view shows parent and children`).
- Any UI surface rendering a span row displays `<RollingDots />` inside a
  `tag warn` chip when `ingestion_state === "placeholder"`, giving an animated
  cue distinct from "complete but empty"
  (`Placeholder ingestion state shown with rolling dots`).
- `kindLabel(KindClass)` rewrites the wire kind for display:
  `execute_tool → "tool"`, `external_tool → "external"`,
  `invoke_agent → "agent"`, `other → "pending"`; any other value is returned
  unchanged. The wire/DB representation is never mutated
  (`Kind badge label renames raw kinds`).
- `hashColor(s)` computes a 32-bit FNV-1a hash (offset basis `0x811c9dc5`,
  prime `0x01000193`), takes `hash % 360` as the hue, and returns
  `hsl(<hue>, 65%, 68%)`. The same input string always produces the same colour
  across reloads, so users can mentally map tool/agent names to hues
  (`Hash color stable hue via FNV-1a`).

### Content & format helpers (content.ts, MessageView, JsonView, CodeBlock)

- `attrs(span)` returns `span.attributes` when it is a non-null object;
  otherwise it `JSON.parse`s `span.attributes_json` and returns the result iff
  it is a plain object; in every other case it returns `{}`. Call sites can
  freely mix the live API shape (`attributes`) with the raw DB shape
  (`attributes_json`) (`Content attrs accepts object or json string`).
- `parseInputMessages(a)` and `parseOutputMessages(a)` accept either an
  already-parsed array or a JSON-stringified array stored at
  `gen_ai.input.messages` / `gen_ai.output.messages`, and yield one `Message`
  per entry: `role` is the entry's string role (or `"unknown"`), `parts` is the
  normalized `parts` array, and the optional `finish_reason` is copied through
  when it is a string (`Content parses input output messages`).
- `parseToolCallArguments(a)` returns `null` when
  `gen_ai.tool.call.arguments` is null/undefined; for a string value it
  `JSON.parse`s and returns the result, falling back to the verbatim string on
  parse failure; non-string values pass through unchanged
  (`Content parses tool call arguments`).
- `parseToolCallResult(a)` returns `null` when `gen_ai.tool.call.result` is
  null/undefined; for a string value it attempts `JSON.parse` and returns the
  parsed object/array, returns the raw string when the parse yields a string
  primitive, and returns the raw string verbatim when parsing throws — bash
  stdout/stderr is never re-encoded (`Content parses tool call result`).
- `hasCapturedContent(a)` is `true` iff at least one of
  `a["gen_ai.input.messages"]`, `a["gen_ai.output.messages"]`, or
  `a["gen_ai.system_instructions"]` is non-null; views consult it to decide
  whether to show the "no content captured" hint
  (`Content has captured content predicate`).
- `fmtNs(ns)` is adaptive: `"—"` when null, `<seconds, two-decimal>"s"` for
  `ms ≥ 1000`, `<ms, one-decimal>"ms"` for `ms ≥ 1`, otherwise `<ns>"ns"`
  (`Content fmtNs adaptive units`).
- `fmtClock(ns)` returns `"—"` when null and otherwise
  `new Date(ns / 1_000_000).toLocaleTimeString()`. `fmtRelative(ns)` returns
  `"—"` when null, `"now"` when the elapsed millisecond delta is negative or
  below 1000, `"<n>s ago"` for `< 60_000`, `"<n>m ago"` for `< 3_600_000`,
  `"<n>h ago"` for `< 86_400_000`, and `"<n>d ago"` otherwise
  (`Content fmtClock and fmtRelative`).
- `prettyJson(v)` returns `JSON.stringify(v, null, 2)` and falls back to
  `String(v)` when stringification throws (e.g. on circular structures); JSON
  helpers must never crash an inspector view (`Content prettyJson safe fallback`).
- `MessageView` renders each `Message` with its `role` as a header and its
  `finish_reason` (when present) as a dim suffix. `PartView` dispatches on
  `part.type`: `text` and `reasoning` render `content` inside `<pre>`,
  `tool_call` renders `name`, short id, and pretty-printed `arguments`,
  `tool_call_response` renders short id plus result (string verbatim or
  pretty-printed JSON), and any unknown `type` is rendered as JSON for
  forward-compatibility with new OTel GenAI part types
  (`Message view renders parts by type`).
- `CodeBlock` renders `Prism.highlight(text, grammar, language)` HTML when
  `Prism.languages[language]` resolves to a grammar, and HTML-escaped text
  otherwise. `langFromPath(path)` returns the matching slug from a fixed
  extension map (e.g. `ts → typescript`, `tsx → tsx`, `py → python`,
  `rs → rust`, `md → markdown`) or from a fixed filename map (`Dockerfile`,
  `Makefile`, `.gitignore`, `.bashrc`, `.zshrc` → `bash`), or `null` for
  unknown paths. Centralizing the map tree-shakes Prism to a curated grammar
  set (`Code block highlights via Prism with extension map`).
- `JsonView` renders `JSON.stringify(value, null, 2)` inside
  `<pre class="json">`; when its `collapsed` prop is true it wraps the `<pre>`
  in a `<details>` element with summary `"json…"`. Stringify failures fall back
  to `String(value)` so cycles never throw
  (`JsonView pretty prints with optional collapse`).

### Context Growth widget

- `ContextGrowthWidget` selects its session as the first column (in
  `useWorkspace().columns` order) whose `config.session` is set, and renders
  `"pick a session"` when no column has a session — the widget is
  workspace-scoped, not column-scoped
  (`Context widget binds to first column session`).
- When grouping context snapshots into chart rows, the widget excludes any
  snapshot whose `span_pk` is not in the set of "top-level" chat span_pks
  computed by walking the session's span tree and admitting only chat spans
  whose `invoke_agent` ancestor depth is `≤ 1`. This avoids double-counting
  sub-agent chat turns against the parent agent's context budget
  (`Context widget excludes sub-agent chats`).
- The chart renders one stacked column per chat turn (one bar per `span_pk`),
  with stacked sub-bars for `input`, `output`, and `reasoning` token counts,
  and overlays a horizontal limit line at the maximum `token_limit` observed
  across rows. The y-axis is `max(maxTokenLimit * 1.05, maxCurrent)` so the
  limit line stays visible even when usage exceeds it
  (`Context widget stack chart per turn`).
- Dragging the resizer at the top of the widget updates
  `contextWidgetHeightVh` to
  `clamp(startVh - (dy / window.innerHeight) * 100, 5, 80)`, where `dy` is the
  pointer's vertical delta from drag start; the `5..80` clamp prevents the
  user from collapsing the widget out of reach or expanding past the viewport
  (`Context widget drag resize 5 to 80 vh`).
- When `contextWidgetVisible` is false the widget renders only a collapsed
  `"▾ context growth"` button that flips the flag back to true; when visible,
  the header exposes a `×` button that hides it again
  (`Context widget hide and show toggle`).
- The chart's stacked column for chat span_pk `p` receives the `hovered` class
  iff `useHoverState().hoveredChatPk === p`, providing cross-component
  highlighting between the Spans column and the widget
  (`Context widget hovered chat highlights matching column`).
- The widget subscribes to
  `useLiveFeed([{ kind: "derived", entity: "chat_turn" },
  { kind: "span", entity: "span" },
  { kind: "span", entity: "placeholder" }])` and, on each `tick` while
  `session` is set, invalidates both `["session-contexts", session]` and
  `["session-span-tree", session]` queries so snapshot and tree state stay
  fresh (`Context widget live invalidation on chat turn and span events`).

## Public surface

Exported components/functions per file (named precisely from the LLRs):

- **`Workspace.tsx`** — `Workspace` (uses internal column resizer; `MIN_COL_PX
  = 120`).
- **`Column.tsx`** — `ColumnBody` (scenario dispatch by `column.scenarioType`).
- **`ColumnHeader.tsx`** — `ColumnHeader` (title input + move-left / move-right /
  remove buttons; renders `"no filters"` when `children` is empty).
- **`content.ts`** — `attrs`, `parseInputMessages`, `parseOutputMessages`,
  `parseToolCallArguments`, `parseToolCallResult`, `hasCapturedContent`,
  `fmtNs`, `fmtClock`, `fmtRelative`, `prettyJson`, plus the `Message` /
  `Part` shape.
- **`ContextGrowthWidget.tsx`** — `ContextGrowthWidget` (consumes
  `useWorkspace`, `useHoverState`, `useLiveFeed`,
  `contextWidgetVisible`/`contextWidgetHeightVh` state).
- **`Inspector.tsx`** — `SpanInspector`, `SpanDetailView`, `RollingDots` (the
  placeholder indicator).
- **`JsonView.tsx`** — `JsonView` (`{ value, collapsed? }`).
- **`KindBadge.tsx`** — `KindBadge`, `kindLabel`, `hashColor` (FNV-1a → HSL).
- **`MessageView.tsx`** — `MessageView`, `PartView` (dispatches on
  `part.type`).
- **`CodeBlock.tsx`** — `CodeBlock`, `langFromPath` (extension + filename map
  → Prism language slug or `null`).

## Invariants & constraints

- **Min column width:** `MIN_COL_PX = 120 px`, enforced before rounding;
  honoured except when total available width is below the sum of all minimums
  (`Workspace minimum column width 120 px`).
- **Resizer is local:** only the two adjacent columns' weights change; their
  pixel sum is preserved; the left side stays in
  `[MIN_COL_PX, total - MIN_COL_PX]`
  (`Resizer drag rebalances two adjacent column weights`).
- **Pixel-exact layout:** integer-pixel rounding with the remainder absorbed
  into the last unpinned column; `4 px` inter-column gutter subtracted before
  distribution (`Workspace lays out columns by weight with ResizeObserver`).
- **Scenario switch is exhaustive in `Column`:** the six scenario types listed
  above are the only place that knows the full set
  (`Column body dispatches by scenario type`).
- **Content parsers tolerate dual shapes:** `attrs`, `parseInputMessages`,
  `parseOutputMessages` accept either parsed-object or JSON-string forms;
  malformed JSON degrades to `{}`, the verbatim string, or an empty list
  rather than throwing (`Content attrs accepts object or json string`,
  `Content parses input output messages`,
  `Content parses tool call arguments`, `Content parses tool call result`).
- **JSON helpers never throw:** `prettyJson` and `JsonView` both fall back to
  `String(value)` on `JSON.stringify` failure (cycles, BigInt, etc.)
  (`Content prettyJson safe fallback`,
  `JsonView pretty prints with optional collapse`).
- **`fmtNs` units:** seconds (≥1000 ms, 2 dp), milliseconds (≥1 ms, 1 dp),
  nanoseconds otherwise; `null → "—"` (`Content fmtNs adaptive units`).
- **Relative-time buckets:** `now` < 1 s, `s` < 1 min, `m` < 1 h, `h` < 1 d,
  `d` thereafter; `null → "—"` (`Content fmtClock and fmtRelative`).
- **`hashColor` stability:** FNV-1a 32-bit (offset basis `0x811c9dc5`, prime
  `0x01000193`), `hash % 360` → `hsl(h, 65%, 68%)`; identical input always
  produces identical colour (`Hash color stable hue via FNV-1a`).
- **`kindLabel` allow-list:** only the four labels above are rewritten;
  unknown kinds are returned verbatim
  (`Kind badge label renames raw kinds`).
- **Context widget bounds:** `contextWidgetHeightVh` is clamped to
  `[5, 80]` vh; chart y-axis is `max(maxTokenLimit * 1.05, maxCurrent)`;
  sub-agent chats (`invoke_agent` ancestor depth `> 1`) are excluded from
  rows (`Context widget drag resize 5 to 80 vh`,
  `Context widget stack chart per turn`,
  `Context widget excludes sub-agent chats`).
- **`MessageView` is forward-compatible:** unknown `part.type` values are
  rendered as JSON rather than swallowed
  (`Message view renders parts by type`).
- **`CodeBlock` fallback:** when `Prism.languages[language]` is missing, text
  is rendered HTML-escaped (no highlight, never raw)
  (`Code block highlights via Prism with extension map`).
- **`SpanInspector` query key:** `["span", trace_id, span_id]` is shared with
  sibling scenarios so the React-Query cache hits across columns
  (`Span inspector fetches and renders detail`).
- **`SpanDetailView` projection block:** entirely omitted when `projection` is
  empty; otherwise every present sub-block is rendered as an open `<details>`
  → `JsonView` (`Span detail view renders projection sub-blocks`).

## Dependencies

- **Reads `api`:** `api.getSpan(trace_id, span_id)` (Inspector); `Span`,
  `SpanDetail`, `SpanRef`, `KindClass`, `Message`/`Part` shape types
  consumed throughout.
- **Reads `state`:**
  - `state/workspace` — `useWorkspace().columns`, `updateColumn`,
    `moveColumn`, `removeColumn`, `contextWidgetVisible`,
    `contextWidgetHeightVh`.
  - `state/hover` — `useHoverState().hoveredChatPk` (cross-column highlight).
  - `state/live` — `useLiveFeed([...])` for chat-turn / span / placeholder
    invalidation in the context widget.
- **Consumed by `app-shell`:** `Workspace` and `ContextGrowthWidget` are
  mounted by the top-level `App` shell; the top bar adds columns that flow
  through `Column` → `ColumnBody`.
- **Consumed by `scenarios/`:** every scenario view embeds `ColumnHeader`
  (chrome), `MessageView`, `JsonView`, `CodeBlock`, `KindBadge`, and
  `SpanInspector` / `SpanDetailView` via the registry resolved in
  `ColumnBody`. `content.ts` formatter and parser helpers are imported
  directly by scenarios that touch span attributes.
- **External libs mentioned in LLRs:**
  - `Prism` (`Prism.highlight`, `Prism.languages[...]`) — `CodeBlock` only,
    with a curated grammar set so the bundle tree-shakes.
  - `ResizeObserver` (browser API) — `Workspace` width measurement.
  - `@tanstack/react-query` — `useQuery` in `SpanInspector` (key
    `["span", trace_id, span_id]`).
  - Browser `Date.toLocaleTimeString()` — `fmtClock`.

## Where to read for detail

- **Vault HLRs:**
  - `frontend/hlr/Workspace Layout.md`
  - `frontend/hlr/Chat detail.md`
  - `frontend/hlr/Tool Call Inspector.md`
  - `frontend/hlr/Trace and Span Explorer.md`
  - `frontend/hlr/Context Growth Widget.md`

- **Vault LLRs (full enumeration of those backed by these components):**
  - `frontend/llr/Workspace lays out columns by weight with ResizeObserver.md`
  - `frontend/llr/Workspace minimum column width 120 px.md`
  - `frontend/llr/Resizer drag rebalances two adjacent column weights.md`
  - `frontend/llr/Empty workspace shows empty state.md`
  - `frontend/llr/Column body dispatches by scenario type.md`
  - `frontend/llr/Column header allows rename move remove.md`
  - `frontend/llr/Content attrs accepts object or json string.md`
  - `frontend/llr/Content parses input output messages.md`
  - `frontend/llr/Content parses tool call arguments.md`
  - `frontend/llr/Content parses tool call result.md`
  - `frontend/llr/Content has captured content predicate.md`
  - `frontend/llr/Content fmtNs adaptive units.md`
  - `frontend/llr/Content fmtClock and fmtRelative.md`
  - `frontend/llr/Content prettyJson safe fallback.md`
  - `frontend/llr/Message view renders parts by type.md`
  - `frontend/llr/Code block highlights via Prism with extension map.md`
  - `frontend/llr/JsonView pretty prints with optional collapse.md`
  - `frontend/llr/Span inspector fetches and renders detail.md`
  - `frontend/llr/Span detail view renders projection sub-blocks.md`
  - `frontend/llr/Span detail view shows parent and children.md`
  - `frontend/llr/Placeholder ingestion state shown with rolling dots.md`
  - `frontend/llr/Kind badge label renames raw kinds.md`
  - `frontend/llr/Hash color stable hue via FNV-1a.md`
  - `frontend/llr/Context widget binds to first column session.md`
  - `frontend/llr/Context widget excludes sub-agent chats.md`
  - `frontend/llr/Context widget stack chart per turn.md`
  - `frontend/llr/Context widget drag resize 5 to 80 vh.md`
  - `frontend/llr/Context widget hide and show toggle.md`
  - `frontend/llr/Context widget hovered chat highlights matching column.md`
  - `frontend/llr/Context widget live invalidation on chat turn and span events.md`

- **Source files (under `web/src/components/`):**
  - `CodeBlock.tsx`
  - `Column.tsx`
  - `ColumnHeader.tsx`
  - `content.ts`
  - `ContextGrowthWidget.tsx`
  - `Inspector.tsx`
  - `JsonView.tsx`
  - `KindBadge.tsx`
  - `MessageView.tsx`
  - `Workspace.tsx`