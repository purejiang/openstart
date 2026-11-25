import type { Run } from "./types";
import { fmtElapsed } from "./format";
import { terminalLabelById } from "./terminals";
import { IconButton } from "./ui";
import { IconStop } from "./icons";

const ACTIVE_STATUSES = new Set(["starting", "running"]);

/** Compact live view of every run that has not finished yet. */
export default function RunsPanel({
  runs,
  stopping,
  onStop,
  onStopAll,
}: {
  runs: Run[];
  stopping: boolean;
  onStop: (runId: string) => void;
  onStopAll: () => void;
}) {
  const active = runs.filter((run) => ACTIVE_STATUSES.has(run.status));
  if (active.length === 0) return null;

  return (
    <section className="runs" aria-label="Active runs">
      <div className="runs-head">
        <span className="runs-title">
          <span className="run-dot running" />
          Running
        </span>
        <span className="runs-count">{active.length}</span>
        <span className="runs-spacer" />
        <button className="btn btn-ghost btn-sm" onClick={onStopAll} disabled={stopping}>
          Stop all
        </button>
      </div>
      {active.map((run) => (
        <div className="run" key={run.id}>
          <span className={`run-dot ${run.status}`} title={run.message || run.status} />
          <span className="run-name" title={run.target_name}>
            {run.target_name}
          </span>
          <span className="run-id" title={`Run ID ${run.id} (CLI: status ${run.id})`}>
            {run.id}
          </span>
          <span className="run-term">{terminalLabelById(run.terminal)}</span>
          <span className="runs-spacer" />
          {run.message && <span className="run-message" title={run.message}>{run.message}</span>}
          <span className="run-pid" title="Process ID">
            {run.pid ? `PID ${run.pid}` : "PID …"}
          </span>
          <span className="run-time" title={`Started ${new Date(run.started_at).toLocaleString()}`}>
            {fmtElapsed(run.started_at)}
          </span>
          <IconButton
            variant="danger"
            label={`Stop ${run.target_name}`}
            disabled={stopping}
            onClick={() => onStop(run.id)}
          >
            <IconStop size={12} />
          </IconButton>
        </div>
      ))}
    </section>
  );
}
