---
type: LLR
tags:
  - req/llr
  - tui
  - domain/spans
---
`chips::target_chips(kind, args)` MUST return `[basename(path)]` when `args.path` or `args.filePath` is a non-empty string (handling both Unix `/` and Windows `\\`/`/` separators with a drive-letter prefix detector); MUST extract one chip per file path via `vendor::copilot::extract_apply_patch_paths(args)` for `ToolKind::Patch`; MUST otherwise return `[hostname(url)]` for `args.url` / `args.target-url` / `args.target_url` strings; MUST return `[]` when `args` is not a JSON object or contains none of these keys. URL hostnames MUST strip user-info, port, and trailing path/query/fragment, and MUST handle bracketed IPv6 hosts.

## Rationale
Files-and-URLs target chip gives each tool span a one-glance subject without paging through `args`.

## Derived from
- [[TUI Spans tree row layout in cells]]
