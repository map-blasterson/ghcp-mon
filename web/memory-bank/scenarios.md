# scenarios — Memory Bank (frontend)

## Purpose

`web/src/scenarios/` is the home of the dashboard's six **scenario components** plus a registry that maps `ScenarioType` → component constructor. A "scenario" is the body of a workspace column: it owns its own data fetching (TanStack Query), its own live-feed subscriptions, and the user interactions that happen inside the column. The surrounding `Workspace` chrome (column headers, resize handles, top bar) is scenario-agnostic and dispatches into a scenario by `column.config.scenarioType`.

The seven files in this folder collectively implement the largest functional area of the frontend — roughly 36 LLRs — and span the full vertical from the backend HTTP API (`api`) through cross-column state (`state/workspace`, `state/live`, `state/hover`) into shared rendering primitives (`components`).

The HLRs covered here are:

- `frontend/hlr/Live Session Browser.md` — `LiveSessions.tsx`
- `frontend/hlr/Trace and Span Explorer.md` — `Spans.tsx`
- `frontend/hlr/Chat detail.md` — `ChatDetail.tsx`
- `frontend/hlr/Tool Call Inspector.md` — `ToolDetail.tsx`
- `frontend/hlr/File Touch Tree.md` — `FileTouches.tsx`
- `frontend/hlr/Raw Record Browser.md` — `RawBrowser.tsx`
- `frontend/hlr/Workspace Layout.md` — `index.ts` (registry portion)

## Key behaviors

### Registry (index.ts)

- Exports a `SCENARIOS` record whose keys are exactly the values of `ScenarioType` and whose values are the matching scenario component constructors. This is a centralized lookup consumed by alternate dispatchers/tests; it complements (does not replace) the explicit switch inside `ColumnBody`. (`frontend/llr/Scenario registry maps scenario type to component.md`)

### Scenario: Live Sessions (LiveSessions.tsx)

- **Data source.** Calls `api.listSessions({ limit: 50 })` and renders one row per `SessionSummary`. Each row shows: name (`local_name` if non-empty, otherwise the first 8 chars of `conversation_id` rendered monospaced); `fmtRelative(last_seen_ns)`; `latest_model` (or `—`); and pluralized `chat_turn_count` / `tool_call_count` / `agent_run_count`. If `branch` is set it is rendered as a chip whose `title` is `cwd ?? undefined`. (`frontend/llr/Live sessions list summary stats.md`)
- **Auto-naming badge.** A row whose `local_name` is non-empty AND `user_named === false` renders an `auto` badge with title `"auto-summarized name (use /rename in copilot to set)"`. (`frontend/llr/Auto-named sessions tagged auto.md`)
- **Click-to-select propagation.** Clicking a row sets `config.session = conversation_id` on its own column AND on every other column whose `scenarioType` is one of `"spans"`, `"chat_detail"`, `"file_touches"`, leaving the other columns' configs otherwise unchanged. (`frontend/llr/Selecting session propagates to dependent columns.md`)
- **Delete.** The per-row delete button calls `confirm("Delete session <8-char-id>? This removes all spans, turns, and tool calls in its trace(s).")`, then on confirm: `api.deleteSession(id)`, clears `config.session` on every column whose `config.session` matches the deleted id, and invalidates the `["sessions"]` query. (`frontend/llr/Delete session confirms and clears column session.md`)
- **Live invalidation.** Subscribes to `useLiveFeed([{kind:"derived", entity:"session"}, {kind:"derived", entity:"chat_turn"}])` and invalidates `["sessions"]` on every live-feed `tick` advance — sessions advance their counters on each chat-turn upsert. (`frontend/llr/Live sessions invalidation on session and chat turn events.md`)

### Scenario: Spans (Spans.tsx)

- **Two modes.** When `column.config.session` is set, renders the `/api/sessions/:cid/span-tree` response as an expandable tree. When unset, renders `/api/traces` as a recent-traces list. Switching `session` clears the column's `selected_trace_id` and `selected_span_id`. (`frontend/llr/Spans scenario two modes session vs traces.md`)
- **Live invalidation.** Subscribes to `useLiveFeed` with seven envelope filters: `(trace,trace)`, `(span,span)`, `(span,placeholder)`, `(derived,tool_call)`, `(derived,chat_turn)`, `(derived,agent_run)`, `(derived,session)`. On each `tick` invalidates `["sessions"]` and either `["session-span-tree", session]` (when session set) or `["traces"]`. (`frontend/llr/Spans live invalidation on ingest events.md`)
- **Selection routing by kind class.** Selecting a span dispatches the new `(selected_trace_id, selected_span_id)` to other columns according to the allow-list `{ spans: "*", tool_detail: ["execute_tool", "external_tool"], chat_detail: ["chat"] }`. A target column receives the selection only if its scenario type is in the map and either the entry is `"*"` or the picked `kind_class` is in the array. Non-applicable columns retain their last applicable selection. (`frontend/llr/Span selection routes by kind class allow list.md`)
- **Follow latest tool span.** While `selected_span_id` matches the previously-latest tool span (kind `execute_tool` or `external_tool`) in the session tree, auto-advances the selection to the new latest tool span when one arrives; stops auto-advancing as soon as the user changes selection away. (`frontend/llr/Spans follows latest tool span.md`)
- **Traces list dimming.** When `kind_filter` is set in trace-list mode, rows whose `kind_counts[kind_filter] === 0` get the `dim` class; they are NOT hidden, so placeholder/partial traces remain visible. (`frontend/llr/Traces list dims rows below kind filter.md`)
- **Hover ancestor publishing.** On `mouseenter` over a span tree row, publishes via `useHoverState.setHoveredChatPk` the `span_pk` of the row's nearest chat ancestor (or itself if it is a chat span), or `null` if no chat ancestor; on `mouseleave` publishes `null`. Drives the cross-component highlight in the Context Growth widget. (`frontend/llr/Span tree row publishes hovered chat ancestor.md`)
- **Skill name chip.** For every span tree row whose
  `projection.tool_call.tool_name === "skill"`, fetches detail via the shared
  `["span", trace_id, span_id]` query (`staleTime: 30_000`), parses
  `gen_ai.tool.call.arguments` via `parseToolCallArguments`, and — when the
  parsed value is a non-array object whose `skill` property is a non-empty
  string — renders `<span class="tag skill">{skill}</span>` on the row. The
  `.tag.skill` style (green diagonal-stripe background) distinguishes it from
  the bash command chips. (`frontend/llr/Skill name chip shows skill argument.md`)
- **Report intent title on parent row.** For every span tree row, scans the
  row's direct children for spans whose
  `projection.tool_call.tool_name === "report_intent"`, picks the one with the
  largest `start_unix_ns ?? span_pk ?? 0` (latest by start time), fetches its
  detail via the shared `["span", trace_id, span_id]` query
  (`staleTime: 30_000`), parses `gen_ai.tool.call.arguments` via
  `parseToolCallArguments`, and — when the parsed value is a non-array object
  whose `intent` property is a non-empty string — renders that text as a
  white-coloured title appended to the parent row
  (`<span style="margin-left: 6px; color: #fff">{intent}</span>`). Surfaces the
  agent's announced intent on the parent (typically `invoke_agent` or `chat`)
  row without expanding it. (`frontend/llr/Report intent title shows on parent row.md`)
- **Bash command chips.** For every span tree row whose `projection.tool_call.tool_name === "bash"`, fetches detail via `["span", trace_id, span_id]`, parses `gen_ai.tool.call.arguments.command`, splits on whitespace-bounded `&&` / `||` / `|`, takes the first non-`KEY=value` token of each segment, basenames it, truncates at 24 chars (with `…`), and renders up to 6 hash-coloured chips with a `…` overflow chip when there are more. (`frontend/llr/Bash command chip extracts primary words.md`)
- **Span inspector.** `SpanInspector` runs `useQuery({ queryKey: ["span", trace_id, span_id], queryFn: () => api.getSpan(...) })`, rendering `"loading…"` while pending, `"span not found"` on error/no-data, and otherwise `SpanDetailView`. The query key is shared with sibling scenarios so the cache hits across columns. (`frontend/llr/Span inspector fetches and renders detail.md`)
- **Detail view — projections.** `SpanDetailView` renders every projection sub-block present on `detail.projection` (`chat_turn`, `tool_call`, `agent_run`, `external_tool_call`) inside an open `<details>` block whose body is a `JsonView` of the projection record; omits the section entirely when `projection` is empty. (`frontend/llr/Span detail view renders projection sub-blocks.md`)
- **Detail view — relations.** Displays the parent as `"<parent.name> (<8-char span_id>)"` (or `—` when none) and lists every child `SpanRef` with its name, `KindBadge`, and 8-char span id. (`frontend/llr/Span detail view shows parent and children.md`)
- **Placeholder ingest cue.** Any UI that renders a span row must display a `<RollingDots />` indicator inside a `tag warn` chip when the row's `ingestion_state === "placeholder"`. (`frontend/llr/Placeholder ingestion state shown with rolling dots.md`)
- **Kind label remapping.** `kindLabel` maps display labels: `execute_tool → "tool"`, `external_tool → "external"`, `invoke_agent → "agent"`, `other → "pending"`, all others passthrough. Wire/DB representation is unchanged. (`frontend/llr/Kind badge label renames raw kinds.md`)

### Scenario: Chat Detail (ChatDetail.tsx)

- **Span-class gate.** Treats the selected span's content as a chat input only when `detail.span.kind_class === "chat"`. For any other class, renders the empty state `"selected span is not a chat span"` and does NOT build the breakdown tree. (`frontend/llr/Chat detail only renders for chat span.md`)
- **Four-branch tree.** The breakdown tree's root contains exactly four child branches, each parsed from a GenAI semconv attribute: `system instructions` (`gen_ai.system_instructions`), `tool definitions` (`gen_ai.tool.definitions`), `input messages` (`gen_ai.input.messages`), `output messages` (`gen_ai.output.messages`). (`frontend/llr/Chat detail tree built from four content attributes.md`)
- **Bytes via JSON length.** Each tree node's `bytes = JSON.stringify(value ?? null).length`, falling back to `0` if `JSON.stringify` throws (cycle-safe). The root's `bytes` equals the sum of its four children. (`frontend/llr/Chat detail bytes computed via JSON length.md`)
- **Proportional summary bar.** A summary bar above the tree renders one segment per currently-visible (collapsed-frontier) node, each segment width set to `(seg.bytes / max(1, totalBytes)) * 100%`. Hovering a tree node marks the corresponding bar segment with the `hovered` class and vice versa. (`frontend/llr/Chat detail summary bar proportional to visible segments.md`)
- **Click-to-expand long primitives.** A primitive value whose stringified length exceeds 200 chars or contains a newline is rendered with the `ib-prim-v-clip` class and toggles open/closed on click. (`frontend/llr/Chat detail long primitives click to expand.md`)

### Scenario: Tool Detail (ToolDetail.tsx)

- **Empty state.** Renders `"selected span is not a tool call"` whenever the resolved `SpanDetail.projection` has neither a `tool_call` nor an `external_tool_call`, regardless of `kind_class`. (`frontend/llr/Tool detail requires tool call projection.md`)
- **Native vs external precedence.** When `projection.tool_call` is populated, renders `ToolDetailBody` and does NOT render `ExternalToolDetailBody`, even if `projection.external_tool_call` is also populated. The native row is richer (carries `tool_type`, `status_code`, drives specialized arg renderers) and takes precedence. (`frontend/llr/Tool detail prefers native tool call over external.md`)
- **Edit tool.** For `tool_name === "edit"`: renders `path` as a kv row, `old_str` as a `CodeBlock` with class `edit-diff edit-diff-old`, `new_str` as a `CodeBlock` with class `edit-diff edit-diff-new`, both highlighted via `langFromPath(path)`; any other arguments rendered as JSON under an `other` label. (`frontend/llr/Edit tool renders old new with syntax highlight.md`)
- **View tool.** For `tool_name === "view"`: when result is a string, strips leading `"<n>. "` line-number prefixes from each line into a separate `<pre class="lns">` gutter and renders the stripped body via `CodeBlock` with `langFromPath(path)`; if no line had a numbered prefix, renders the body without a gutter. Prism cannot highlight content with the literal line-number prefix. (`frontend/llr/View tool splits line numbers into gutter.md`)
- **Task tool.** For `tool_name === "task"`: renders the string `prompt` argument as Markdown via `react-markdown` + `remark-gfm`; every other argument as a kv row. (`frontend/llr/Task tool renders prompt as markdown.md`)
- **Read agent tool.** For `tool_name === "read_agent"`: arguments rendered as a kv list; when result is a string, rendered as Markdown via `react-markdown` + `remark-gfm`; non-string results fall back to pretty-printed JSON. (`frontend/llr/Read agent tool renders result as markdown.md`)
- **Generic fallback.** For any other tool: splits parsed arguments into "code-ish" string fields (any string containing a newline) vs the remainder. Code-ish fields rendered as `<pre class="edit-diff">`; remainder rendered as a single pretty-printed JSON block. A string `result` rendered as `<pre>`; non-string result rendered as pretty-printed JSON. (`frontend/llr/Generic tool renders args splitting code-ish strings.md`)
- **No content captured.** When a tool span has neither parsed arguments nor a parsed result, the body renders the literal `NO_CONTENT_LINE`: `"no content captured — set OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true and OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental"`. (`frontend/llr/Tool detail empty state when no content captured.md`)
- **External tool detail header.** When routed to `ExternalToolDetailBody` (i.e. `projection.tool_call` absent, `external_tool_call` present), the header is an `<h4>` showing `ext.tool_name ?? "(unknown tool)"` followed by a kv block in this exact order: `call_id` (`ext.call_id ?? "—"`), `tool_type` (literal `"external"`), `duration` (`fmtNs(span.duration_ns ?? (span.end_unix_ns - span.start_unix_ns))`, `fmtNs(null)` if a bound is missing), `start` (`fmtClock(span.start_unix_ns)`), `conv` (first 8 chars of `ext.conversation_id`, else `"—"`), `paired_tool_call_pk` (`ext.paired_tool_call_pk ?? "—"`), `agent_run_pk` (`ext.agent_run_pk ?? "—"`). (`frontend/llr/External tool detail body header fields.md`)
- **External tool args dispatch.** `ExternalToolDetailBody` renders its args/result section using `GenericArgs` for every external tool, regardless of `ext.tool_name`. It does NOT dispatch to the specialized `EditArgs` / `ViewArgs` / `ReadAgentArgs` / `TaskArgs` renderers — those names are CLI-specific and external tool calls (MCP servers etc.) carry arbitrary, server-defined names. (`frontend/llr/External tool detail uses generic args renderer.md`)

### Scenario: File Touches (FileTouches.tsx)

- **Aggregation source.** Queries `api.listSpans({ session, kind: "execute_tool", limit: 1000 })`, keeps only spans whose name parses as `"execute_tool <tool_name>"` with `tool_name ∈ {"view", "edit", "create"}`, and classifies `view → "read"`, `edit`/`create → "write"`. (`frontend/llr/File touches aggregates view edit create.md`)
- **Tree build.** For every matching span, fetches its detail under the shared `["span", trace_id, span_id]` key, extracts the string `path` argument (skipping spans without one), splits on `/` (collapsing repeated separators, trimming trailing `/`), and inserts a `Touch` along the path. Each ancestor directory increments its `reads` or `writes` counter; the leaf node appends to `fileTouches`. The shared query key reuses cache populated by ToolDetail and ChatDetail. (`frontend/llr/File touches builds filesystem tree with counts.md`)
- **New directories open by default.** When a directory path appears in the touched tree for the first time, it is added to the open-directory set. Already-known directories preserve their current open/closed state across live updates — newly-discovered files default to visible while honoring explicit user collapses. (`frontend/llr/File touches new directories open by default.md`)
- **Sort directories first.** Children of any tree node are sorted with directories (`children.size > 0`) before files; within each group, alphabetically by `name` via `localeCompare`. (`frontend/llr/File touches sort directories first then alphabetical.md`)
- **Expand/collapse all.** Column header exposes `[+]` and `[-]` buttons that respectively set the open-dir set to the full set of directory paths or to the empty set. Both `disabled` when no directories are present. (`frontend/llr/File touches expand and collapse all controls.md`)
- **Live invalidation.** Subscribes to `useLiveFeed([{kind:"span", entity:"span"}, {kind:"derived", entity:"tool_call"}])` and on each `tick` invalidates the session-scoped `["spans", { session, kind: "execute_tool", limit: 1000 }]` query. (`frontend/llr/File touches live invalidation on tool events.md`)

### Scenario: Raw Browser (RawBrowser.tsx)

- **Filterable list + JSON detail.** Queries `api.listRaw({ type, limit: 200 })` (where `type = column.config.raw_type` or `undefined`), renders one row per `RawRecord` showing `record_type`, `#<id>`, and `received_at`, and renders the selected record's `body` via `JsonView` in a detail pane. Direct surface over `/api/raw` for debugging the persistence layer. (`frontend/llr/Raw browser lists records filterable by type.md`)
- **Live invalidation.** Subscribes to `useLiveFeed([{kind:"span", entity:"span"}, {kind:"metric", entity:"metric"}])` and invalidates `["raw", t]` on every `tick`. Both span and metric ingestion produce raw records. (`frontend/llr/Raw browser live invalidation on span and metric events.md`)

## Public surface

| Scenario | Exported component | File |
|---|---|---|
| Live Sessions | `LiveSessionsScenario` | `LiveSessions.tsx` |
| Spans | `SpansScenario` | `Spans.tsx` |
| Chat Detail | `ChatDetailScenario` | `ChatDetail.tsx` |
| Tool Detail | `ToolDetailScenario` | `ToolDetail.tsx` |
| File Touches | `FileTouchesScenario` | `FileTouches.tsx` |
| Raw Browser | `RawBrowserScenario` | `RawBrowser.tsx` |
| Registry | `SCENARIOS: Record<ScenarioType, ComponentCtor>` | `index.ts` |

The registry is the canonical lookup; `ColumnBody` carries an explicit switch that mirrors it.

## Invariants & constraints

- **Selection routing by kind-class allow-list (Spans → others).** Map: `{ spans: "*", tool_detail: ["execute_tool", "external_tool"], chat_detail: ["chat"] }`. Non-matching columns retain their last applicable selection so multiple inspectors can stay live. (`frontend/llr/Span selection routes by kind class allow list.md`)
- **Selection routing by scenario type (LiveSessions → others).** Session click propagates `config.session` only to columns of type `"spans"`, `"chat_detail"`, `"file_touches"`. (`frontend/llr/Selecting session propagates to dependent columns.md`)
- **Live invalidation triggers per scenario.**
  - LiveSessions: `(derived,session)`, `(derived,chat_turn)` → `["sessions"]`.
  - Spans: `(trace,trace)`, `(span,span)`, `(span,placeholder)`, `(derived,tool_call)`, `(derived,chat_turn)`, `(derived,agent_run)`, `(derived,session)` → `["sessions"]` + `["session-span-tree", session]` or `["traces"]`.
  - FileTouches: `(span,span)`, `(derived,tool_call)` → session-scoped spans list.
  - RawBrowser: `(span,span)`, `(metric,metric)` → `["raw", t]`.
  - ChatDetail / ToolDetail: rely on the shared `["span", trace_id, span_id]` cache populated by Spans; do not own a primary live subscription.
- **Auto-named sessions.** A row gets the `auto` badge iff `local_name` non-empty AND `user_named === false`.
- **Follow-latest behavior (Spans).** Auto-advance to the newest tool span only while the user's `selected_span_id` matches the previous latest tool span; user reselection halts the follow. (`frontend/llr/Spans follows latest tool span.md`)
- **Empty-state rules.**
  - ChatDetail: `"selected span is not a chat span"` when `kind_class !== "chat"`.
  - ToolDetail: `"selected span is not a tool call"` when neither projection populated; `NO_CONTENT_LINE` when neither args nor result captured.
  - Spans inspector: `"loading…"` pending, `"span not found"` on error or empty data.
  - FileTouches expand/collapse buttons: `disabled` when there are no directories.
  - TracesList: rows whose `kind_counts[kind_filter] === 0` are dimmed, never hidden.
- **Native-vs-external precedence (ToolDetail).** `projection.tool_call` always wins over `external_tool_call` when both are present. `ExternalToolDetailBody` always uses `GenericArgs`; specialized renderers (`EditArgs`/`ViewArgs`/`TaskArgs`/`ReadAgentArgs`) are reserved for native tool calls.
- **Shared query keys.** `["span", trace_id, span_id]` is the canonical cache key reused by Spans, ToolDetail, ChatDetail, FileTouches, and the bash-chip lookup. `["sessions"]`, `["session-span-tree", session]`, `["traces"]`, `["raw", t]`, `["spans", { session, kind, limit }]` are the per-scenario primary keys.

## Dependencies

- **`api`** — `listSessions`, `deleteSession`, `getSpan`, `listSpans`, `listRaw`, plus span-tree / traces endpoints.
- **`state/workspace`** — column config mutation for selection propagation (session, trace_id, span_id, scenarioType).
- **`state/live`** — `useLiveFeed(filters)` returning a `tick` integer that drives query invalidation.
- **`state/hover`** — `useHoverState.setHoveredChatPk` consumed by Spans (publish) and the Context Growth widget (subscribe).
- **`components`** — `JsonView`, `CodeBlock`, `KindBadge`, `RollingDots`, `MessageView`, `langFromPath`, hash-color, `fmtRelative` / `fmtClock` / `fmtNs`, generic kv block.
- **TanStack Query** (`useQuery`, `useQueryClient`) for caching/invalidation. Bootstrap is in `App.tsx` per `frontend/llr/App bootstraps React StrictMode and TanStack QueryClient.md` and `frontend/llr/Default TanStack Query options no refetch on focus.md`.
- **`react-markdown` + `remark-gfm`** for `task.prompt` and `read_agent` string results.
- **Prism** (via `CodeBlock`) for syntax highlighting in `edit` / `view` and language-aware code blocks; see `frontend/llr/Code block highlights via Prism with extension map.md`.

## Notes

All three external/native-precedence ToolDetail LLRs declare `Derived from: frontend/hlr/Tool Call Inspector.md` in frontmatter. (Earlier transient discovery flagged them as orphaned because they were absent from the HLR's `## Derived LLRs` enumeration; the actual `Derived from` edges are intact.)

- `frontend/llr/External tool detail body header fields.md`
- `frontend/llr/External tool detail uses generic args renderer.md`
- `frontend/llr/Tool detail prefers native tool call over external.md`

Functionally these three LLRs are essential to ToolDetail's behavior (native-vs-external precedence and the entire `ExternalToolDetailBody` contract) and have been folded into the "Scenario: Tool Detail" bullets above.

## Where to read for detail

### Vault HLRs

- `frontend/hlr/Live Session Browser.md`
- `frontend/hlr/Trace and Span Explorer.md`
- `frontend/hlr/Chat detail.md`
- `frontend/hlr/Tool Call Inspector.md`
- `frontend/hlr/File Touch Tree.md`
- `frontend/hlr/Raw Record Browser.md`
- `frontend/hlr/Workspace Layout.md`

### Vault LLRs

**LiveSessions.tsx**
- `frontend/llr/Live sessions list summary stats.md`
- `frontend/llr/Selecting session propagates to dependent columns.md`
- `frontend/llr/Delete session confirms and clears column session.md`
- `frontend/llr/Auto-named sessions tagged auto.md`
- `frontend/llr/Live sessions invalidation on session and chat turn events.md`

**Spans.tsx**
- `frontend/llr/Spans scenario two modes session vs traces.md`
- `frontend/llr/Spans live invalidation on ingest events.md`
- `frontend/llr/Span selection routes by kind class allow list.md`
- `frontend/llr/Spans follows latest tool span.md`
- `frontend/llr/Traces list dims rows below kind filter.md`
- `frontend/llr/Span tree row publishes hovered chat ancestor.md`
- `frontend/llr/Bash command chip extracts primary words.md`
- `frontend/llr/Skill name chip shows skill argument.md`
- `frontend/llr/Report intent title shows on parent row.md`
- `frontend/llr/Span inspector fetches and renders detail.md`
- `frontend/llr/Span detail view renders projection sub-blocks.md`
- `frontend/llr/Span detail view shows parent and children.md`
- `frontend/llr/Placeholder ingestion state shown with rolling dots.md`
- `frontend/llr/Kind badge label renames raw kinds.md`
- `frontend/llr/Hash color stable hue via FNV-1a.md` (shared utility)

**ChatDetail.tsx**
- `frontend/llr/Chat detail only renders for chat span.md`
- `frontend/llr/Chat detail tree built from four content attributes.md`
- `frontend/llr/Chat detail bytes computed via JSON length.md`
- `frontend/llr/Chat detail summary bar proportional to visible segments.md`
- `frontend/llr/Chat detail long primitives click to expand.md`
- Plus the `Content *` family that ChatDetail consumes from `components/`.

**ToolDetail.tsx**
- `frontend/llr/Tool detail requires tool call projection.md`
- `frontend/llr/Edit tool renders old new with syntax highlight.md`
- `frontend/llr/View tool splits line numbers into gutter.md`
- `frontend/llr/Task tool renders prompt as markdown.md`
- `frontend/llr/Read agent tool renders result as markdown.md`
- `frontend/llr/Generic tool renders args splitting code-ish strings.md`
- `frontend/llr/Tool detail empty state when no content captured.md`
- `frontend/llr/External tool detail body header fields.md`
- `frontend/llr/External tool detail uses generic args renderer.md`
- `frontend/llr/Tool detail prefers native tool call over external.md`

**FileTouches.tsx**
- `frontend/llr/File touches aggregates view edit create.md`
- `frontend/llr/File touches builds filesystem tree with counts.md`
- `frontend/llr/File touches new directories open by default.md`
- `frontend/llr/File touches sort directories first then alphabetical.md`
- `frontend/llr/File touches expand and collapse all controls.md`
- `frontend/llr/File touches live invalidation on tool events.md`

**RawBrowser.tsx**
- `frontend/llr/Raw browser lists records filterable by type.md`
- `frontend/llr/Raw browser live invalidation on span and metric events.md`

**index.ts (registry)**
- `frontend/llr/Scenario registry maps scenario type to component.md`

### Source files

- `web/src/scenarios/index.ts` — registry export
- `web/src/scenarios/LiveSessions.tsx`
- `web/src/scenarios/Spans.tsx`
- `web/src/scenarios/ChatDetail.tsx`
- `web/src/scenarios/ToolDetail.tsx`
- `web/src/scenarios/FileTouches.tsx`
- `web/src/scenarios/RawBrowser.tsx`