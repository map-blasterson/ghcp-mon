import type { Column } from "../state/workspace";
import { LiveSessionsScenario } from "../scenarios/LiveSessions";
import { SpansScenario } from "../scenarios/Spans";
import { ToolDetailScenario } from "../scenarios/ToolDetail";
import { RawBrowserScenario } from "../scenarios/RawBrowser";
import { ChatDetailScenario } from "../scenarios/ChatDetail";
import { FileTouchesScenario } from "../scenarios/FileTouches";

interface Props { column: Column; }

export function ColumnBody({ column }: Props) {
  switch (column.scenarioType) {
    case "live_sessions": return <LiveSessionsScenario column={column} />;
    case "spans": return <SpansScenario column={column} />;
    case "tool_detail": return <ToolDetailScenario column={column} />;
    case "raw_browser": return <RawBrowserScenario column={column} />;
    case "input_breakdown": return <ChatDetailScenario column={column} />;
    case "file_touches": return <FileTouchesScenario column={column} />;
  }
}
