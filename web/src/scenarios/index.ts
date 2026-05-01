import type { ScenarioType } from "../state/workspace";
import { LiveSessionsScenario } from "./LiveSessions";
import { SpansScenario } from "./Spans";
import { ToolDetailScenario } from "./ToolDetail";
import { RawBrowserScenario } from "./RawBrowser";
import { InputBreakdownScenario } from "./InputBreakdown";
import { FileTouchesScenario } from "./FileTouches";

import type { Column } from "../state/workspace";

type ScenarioComp = (props: { column: Column }) => JSX.Element;

export const SCENARIOS: Record<ScenarioType, ScenarioComp> = {
  live_sessions: LiveSessionsScenario,
  spans: SpansScenario,
  tool_detail: ToolDetailScenario,
  raw_browser: RawBrowserScenario,
  input_breakdown: InputBreakdownScenario,
  file_touches: FileTouchesScenario,
};
