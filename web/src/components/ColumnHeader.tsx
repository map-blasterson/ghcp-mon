import { useWorkspace, type Column } from "../state/workspace";

interface Props {
  column: Column;
  children?: React.ReactNode;
}

export function ColumnHeader({ column, children }: Props) {
  const { updateColumn, removeColumn, moveColumn } = useWorkspace();
  return (
    <>
      <div className="col-header">
        <input
          className="title"
          value={column.title}
          style={{ background: "transparent", border: "none", flex: "0 1 auto", minWidth: 40 }}
          onChange={(e) => updateColumn(column.id, { title: e.target.value })}
        />
        <span className="actions">
          <button onClick={() => moveColumn(column.id, -1)} title="Move left">←</button>
          <button onClick={() => moveColumn(column.id, 1)} title="Move right">→</button>
          <button onClick={() => removeColumn(column.id)} title="Remove">×</button>
        </span>
      </div>
      {children ? <div className="col-config">{children}</div> : <div className="col-config muted">no filters</div>}
    </>
  );
}
