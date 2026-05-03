import { Workspace } from "./components/Workspace";
import { ContextGrowthWidget } from "./components/ContextGrowthWidget";
import { useWsStatus } from "./state/live";
import { useWorkspace, SCENARIO_LABELS, genId, type ScenarioType } from "./state/workspace";

export function App() {
  const connected = useWsStatus();
  const addColumn = useWorkspace((s) => s.addColumn);
  const reset = useWorkspace((s) => s.resetDefault);

  const onAdd = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const t = e.target.value as ScenarioType | "";
    if (!t) return;
    addColumn({
      id: genId(),
      scenarioType: t,
      title: SCENARIO_LABELS[t],
      config: {},
      width: 1.2,
    });
    e.target.value = "";
  };

  return (
    <div className="app">
      <div className="topbar">
        <span className="brand">ghcp-mon</span>
        <span className="dim">realtime copilot OTel inspector</span>
        <span className="spacer" />
        <select onChange={onAdd} defaultValue="" title="Add column">
          <option value="">+ add column…</option>
          {Object.entries(SCENARIO_LABELS).map(([k, v]) => (
            <option key={k} value={k}>{v}</option>
          ))}
        </select>
        <button onClick={reset} title="Reset to default workspace">reset</button>
        <span className="dim">{connected ? "connected" : "disconnected"}</span>
        <span className={`status-dot${connected ? " on" : ""}`} title={connected ? "connected" : "disconnected"} />
      </div>
      <Workspace />
      <ContextGrowthWidget />
    </div>
  );
}
