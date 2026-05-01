import type { Message, Part } from "./content";
import { prettyJson } from "./content";

interface Props {
  message: Message;
}

/** Render a single GenAI message (one entry from gen_ai.input.messages /
 *  gen_ai.output.messages). Renders each part by its type discriminator
 *  per the input/output messages JSON schemas. */
export function MessageView({ message }: Props) {
  const role = message.role;
  return (
    <div className={`msg ${role}`}>
      <div className="role">
        {role}
        {message.finish_reason && (
          <span className="dim" style={{ marginLeft: 8, fontWeight: "normal" }}>
            finish_reason: {message.finish_reason}
          </span>
        )}
      </div>
      {message.parts.length === 0 ? (
        <div className="no-content">(no parts)</div>
      ) : (
        message.parts.map((p, i) => <PartView key={i} part={p} />)
      )}
    </div>
  );
}

export function PartView({ part }: { part: Part }) {
  switch (part.type) {
    case "text":
      return <div className="body"><pre className="msg-text">{(part as { content: string }).content}</pre></div>;
    case "reasoning":
      return (
        <div className="body reasoning" style={{ opacity: 0.8, fontStyle: "italic", borderLeft: "2px solid var(--border)", paddingLeft: 8, marginTop: 4 }}>
          <span className="dim" style={{ fontStyle: "normal" }}>[reasoning] </span>
          <pre className="msg-text" style={{ display: "inline-block", margin: 0, fontStyle: "italic" }}>
            {(part as { content: string }).content}
          </pre>
        </div>
      );
    case "tool_call": {
      const tc = part as { id: string; name: string; arguments: unknown };
      return (
        <div className="body tool-call" style={{ marginTop: 4 }}>
          <div className="dim">
            <span className="tag">tool_call</span> <span className="mono">{tc.name}</span>
            {tc.id && <span className="mono"> · id={shortId(tc.id)}</span>}
          </div>
          <pre className="json">{prettyJson(tc.arguments)}</pre>
        </div>
      );
    }
    case "tool_call_response": {
      const tr = part as { id: string; response: unknown };
      return (
        <div className="body tool-response" style={{ marginTop: 4 }}>
          <div className="dim">
            <span className="tag">tool_call_response</span>
            {tr.id && <span className="mono"> · id={shortId(tr.id)}</span>}
          </div>
          {typeof tr.response === "string" ? (
            <pre className="msg-text">{tr.response}</pre>
          ) : (
            <pre className="json">{prettyJson(tr.response)}</pre>
          )}
        </div>
      );
    }
    default:
      return (
        <div className="body unknown-part" style={{ marginTop: 4 }}>
          <div className="dim"><span className="tag">{part.type}</span></div>
          <pre className="json">{prettyJson(part)}</pre>
        </div>
      );
  }
}

export function MessageList({ messages }: { messages: Message[] }) {
  return (
    <>
      {messages.map((m, i) => (
        <MessageView key={i} message={m} />
      ))}
    </>
  );
}

function shortId(id: string): string {
  if (id.length <= 12) return id;
  return id.slice(0, 6) + "…" + id.slice(-4);
}
