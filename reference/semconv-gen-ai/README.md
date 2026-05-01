# OpenTelemetry Semantic Conventions — Gen AI (local mirror)

Source: <https://github.com/open-telemetry/semantic-conventions>
Pinned commit: see `VERSION` (raw files fetched from `raw.githubusercontent.com` at that SHA).

Rendered docs: <https://opentelemetry.io/docs/specs/semconv/gen-ai/>

## Layout

- `model/` — authoritative Weaver YAML model (`registry.yaml`, `spans.yaml`, `metrics.yaml`, `events.yaml`).
  These define attributes, types, requirement levels, stability, and groups; they are the machine-readable
  source of truth used to generate the markdown spec.
- `docs/` — rendered markdown spec plus JSON Schemas for message/tool payloads:
  - `gen-ai-spans.md`, `gen-ai-agent-spans.md`, `gen-ai-events.md`, `gen-ai-metrics.md`,
    `gen-ai-exceptions.md`
  - JSON Schemas: `gen-ai-input-messages.json`, `gen-ai-output-messages.json`,
    `gen-ai-system-instructions.json`, `gen-ai-tool-definitions.json`,
    `gen-ai-retrieval-documents.json`
  - Provider-specific bindings: `anthropic.md`, `openai.md`, `aws-bedrock.md`,
    `azure-ai-inference.md`, `mcp.md`

## Refresh

```bash
REF=$(cat reference/semconv-gen-ai/VERSION)   # or pick a newer commit/tag
BASE=https://raw.githubusercontent.com/open-telemetry/semantic-conventions/$REF
for f in events.yaml metrics.yaml registry.yaml spans.yaml; do
  curl -sSfL "$BASE/model/gen-ai/$f" -o "reference/semconv-gen-ai/model/$f"
done
# (same loop for docs/gen-ai/* — see this README's file list)
```
