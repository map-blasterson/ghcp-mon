---
name: update-requirements-from-todays-changes
description: >-
  Workflow for updating the ghcp-mon Obsidian requirements vault from today's
  source-code changes. Lists files modified today under `./src` and `./web/src`,
  then runs the `reverse-engineer` agent in parallel against the backend and
  frontend slices to create or update HLR/LLR/impl notes under the `backend`
  and `frontend` vault scopes. Use when the user asks to "update requirements
  from today's changes", "sync today's code into the vault", or any equivalent
  request that combines a `git`-based daily file selection with the
  reverse-engineer agent and the backend/frontend scope split used by this
  repo.
user-invocable: false
---

# Update Requirements From Today's Changes

This skill documents the recurring workflow for keeping the Obsidian
requirements vault in sync with day-to-day source-code edits in this repo.

## Repo layout assumptions

- **Backend** code lives under `./src` (Rust crate rooted at `Cargo.toml`).
  Backend requirements live under the `backend/` scope of the Obsidian vault
  (e.g. `backend/hlr/`, `backend/llr/`, `backend/impl/`).
- **Frontend** code lives under `./web/src` (Vite/React/TypeScript app rooted
  at `web/package.json`). Frontend requirements live under the `frontend/`
  scope of the vault (e.g. `frontend/hlr/`, `frontend/llr/`, `frontend/impl/`).

The vault is served by the `obsidian_*` tools and follows the
`obsidian-datamodel` skill. Reverse engineering is performed by the
`reverse-engineer` custom agent.

## Procedure

### 1. Enumerate files touched today

Use `git log` scoped to today (local time) to list files modified in the two
trees. Do **not** rely on filesystem mtimes — they are unreliable across
checkouts and rebases.

```bash
TODAY=$(date +%Y-%m-%d)
TOMORROW=$(date -d "$TODAY +1 day" +%Y-%m-%d)

git log --since="$TODAY 00:00:00" --until="$TOMORROW 00:00:00" \
    --name-only --pretty=format: \
  | sort -u | grep -v '^$'
```

Partition the result into:

- **backend set** — paths matching `^src/`
- **frontend set** — paths matching `^web/src/`

Ignore everything else (`Cargo.lock`, `web/package-lock.json`, `dist/`,
generated files, top-level docs, etc.) for this workflow — those don't carry
requirements.

If both sets are empty, stop and tell the user there is nothing to update.

### 2. Launch the two reverse-engineer agents in parallel

Start **two** `reverse-engineer` agents in the **same response**, in
`background` mode, so they run concurrently:

- Agent A — name `re-backend`, scope `backend`, sources = the backend set.
- Agent B — name `re-frontend`, scope `frontend`, sources = the frontend set.

Each prompt MUST include:

1. The absolute repo root (e.g. `/home/<user>/git/ghcp-mon` — use the
   current working directory) so the agent can read source.
2. The exact list of repo-relative source paths to process.
3. The target vault scope (`backend` or `frontend`).
4. An instruction to **create or update** HLR/LLR/impl notes — never
   duplicate. Existing impl notes whose `source:` frontmatter already points
   at the file MUST be reused.
5. A reminder to follow the `obsidian-datamodel` skill: typed frontmatter,
   tag taxonomy, requirement DAG, RFC-2119 normative LLR statements,
   `## Source For` lists on impl notes, and `## Derived from` links on
   LLRs/HLRs.

### 3. Wait, then verify

After both agents report `idle`, read their results with `read_agent`. Then
run:

- `obsidian_find_gaps kind=unresolved` — must report no broken wikilinks.
- Spot-check: for each modified source, confirm there is exactly one
  `type: impl` note with a matching `source:` property, and that its
  `## Source For` list reflects the latest behaviors.

### 4. When NOT to use this skill

- The user asks to reverse-engineer a **specific** file or feature — call
  the `reverse-engineer` agent directly with their explicit input instead.
- The user asks to refine, tag, or test existing requirements — those have
  their own dedicated agents (`refiner`, `tagger`, `tester`/`tester-rust`).
- The repo layout no longer matches the assumptions above. Update this
  skill before relying on it.

## Notes

- The split is enforced **by source path**, not by tag. The
  `reverse-engineer` agent decides tagging within each scope.
- `Cargo.lock` and other generated files appearing in the daily diff are
  expected and are dropped by the path filter in step 1.
- Running the two agents in parallel is the whole point of this skill —
  they touch disjoint vault subtrees (`backend/` vs `frontend/`) and have
  no link conflicts, so serial execution would just waste wall-clock time.
