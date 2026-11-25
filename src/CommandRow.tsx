import type { Command, Run, TerminalInfo } from "./types";
import { fmtDateTime, fmtRelative } from "./format";
import { terminalAccent, terminalLabel } from "./terminals";
import { IconButton, Switch } from "./ui";
import { IconChevronDown, IconEdit, IconPlay, IconStop, IconTrash } from "./icons";

export default function CommandRow({
  command,
  terminals,
  activeRun,
  lastRun,
  expanded,
  stopping,
  toggling,
  onToggleEnabled,
  onRun,
  onStop,
  onEdit,
  onDelete,
  onToggleSteps,
}: {
  command: Command;
  terminals: TerminalInfo[];
  activeRun?: Run;
  lastRun?: Run;
  expanded: boolean;
  stopping: boolean;
  toggling: boolean;
  onToggleEnabled: (command: Command) => void;
  onRun: (command: Command) => void;
  onStop: (runId: string) => void;
  onEdit: (command: Command) => void;
  onDelete: (command: Command) => void;
  onToggleSteps: (id: string) => void;
}) {
  const terminal = terminals.find((t) => t.id === command.terminal) ?? {
    id: command.terminal,
    name: command.terminal,
    path: "",
    available: true,
  };
  const steps = command.steps.length > 0 ? command.steps : [{ cmd: command.command, delay_sec: 0, note: "" }];
  const firstStep = steps[0];
  const extraSteps = steps.slice(1);
  const running = Boolean(activeRun);
  const failed = lastRun?.status === "failed";
  const lastAt = lastRun?.started_at || command.last_executed_at;

  return (
    <div
      className={`cmd-row${command.enabled ? "" : " is-disabled"}${running ? " is-running" : ""}`}
    >
      <div className="cmd-switch">
        <Switch
          checked={command.enabled}
          label={`${command.enabled ? "Disable" : "Enable"} ${command.name}`}
          onChange={() => onToggleEnabled(command)}
          disabled={toggling}
        />
      </div>

      <div className="cmd-main">
        <div className="cmd-title">
          <span className="cmd-name" title={command.name}>
            {command.name}
          </span>
          {command.auto_start && <span className="chip chip-auto">AUTO</span>}
          {running && activeRun && (
            <span className="cmd-running" title={activeRun.message || "Running"}>
              <span className={`run-dot ${activeRun.status}`} />
              {activeRun.status === "starting" ? "Starting" : "Running"}
              {activeRun.pid ? <span className="cmd-pid">PID {activeRun.pid}</span> : null}
            </span>
          )}
          {command.note && (
            <span className="cmd-note" title={command.note}>
              {command.note}
            </span>
          )}
        </div>

        <div className="cmd-line">
          <span className="cmd-text" title={firstStep?.cmd || command.command}>
            {firstStep?.cmd || command.command}
          </span>
          {firstStep?.note && <span className="cmd-step-note">{firstStep.note}</span>}
          {extraSteps.length > 0 && (
            <button
              type="button"
              className="cmd-steps-btn"
              aria-expanded={expanded}
              onClick={() => onToggleSteps(command.id)}
              title={expanded ? "Hide steps" : "Show all steps"}
            >
              {steps.length} steps
              <IconChevronDown size={11} />
            </button>
          )}
        </div>

        {expanded && extraSteps.length > 0 && (
          <ol className="cmd-steps">
            {extraSteps.map((step, index) => (
              <li key={index} className="cmd-step">
                <span className="cmd-step-index">{index + 2}</span>
                <span className="cmd-step-text" title={step.cmd}>
                  {step.cmd}
                </span>
                {step.note && <span className="cmd-step-note">{step.note}</span>}
              </li>
            ))}
          </ol>
        )}
      </div>

      <div className="cmd-side">
        <span className="term" title={terminalLabel(terminal)}>
          <span className={`term-dot ${terminalAccent(command.terminal)}`} />
          <span className="term-label">{terminalLabel(terminal)}</span>
        </span>
        <span
          className={`cmd-last${failed ? " is-failed" : ""}`}
          title={lastAt ? `Last run: ${fmtDateTime(lastAt)}${lastRun?.message ? ` — ${lastRun.message}` : ""}` : "Never run"}
        >
          {failed ? `Failed ${fmtRelative(lastAt)}` : fmtRelative(lastAt)}
        </span>
        <div className="cmd-actions">
          {running && activeRun ? (
            <IconButton
              variant="danger"
              label={`Stop ${command.name}`}
              disabled={stopping}
              onClick={() => onStop(activeRun.id)}
            >
              <IconStop size={12} />
            </IconButton>
          ) : (
            <IconButton
              variant="accent"
              label={command.enabled ? `Run ${command.name}` : `${command.name} is disabled`}
              disabled={stopping || !command.enabled}
              onClick={() => onRun(command)}
            >
              <IconPlay size={13} />
            </IconButton>
          )}
          <IconButton label={`Edit ${command.name}`} onClick={() => onEdit(command)}>
            <IconEdit size={14} />
          </IconButton>
          <IconButton
            className="icon-btn-hover-danger"
            label={`Delete ${command.name}`}
            onClick={() => onDelete(command)}
          >
            <IconTrash size={14} />
          </IconButton>
        </div>
      </div>
    </div>
  );
}
