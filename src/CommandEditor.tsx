import { useState } from "react";
import { DragDropContext, Droppable, Draggable } from "@hello-pangea/dnd";
import type { DropResult } from "@hello-pangea/dnd";
import type { Command, CommandStep, TerminalInfo } from "./types";
import { addCommand, errorMessage, updateCommand } from "./api";
import { fmtDateTime } from "./format";
import { terminalLabel } from "./terminals";
import { IconButton, Modal, Switch } from "./ui";
import { IconGrip, IconPlus, IconX } from "./icons";

type DraftStep = CommandStep;

/* ── Group combobox (pick an existing group or type a new one) ──────── */

function GroupSelect({
  value,
  groups,
  onChange,
}: {
  value: string;
  groups: string[];
  onChange: (value: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [focused, setFocused] = useState(-1);
  const query = value.trim().toLowerCase();
  const options = groups.filter((group) => group.toLowerCase().includes(query) && group !== value);
  const canCreate = value.trim().length > 0 && !groups.includes(value.trim());

  function select(group: string) {
    onChange(group);
    setOpen(false);
    setFocused(-1);
  }

  function onKeyDown(event: React.KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setOpen(true);
      setFocused((index) => Math.min(index + 1, options.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setFocused((index) => Math.max(index - 1, -1));
    } else if (event.key === "Enter" && open) {
      event.preventDefault();
      if (focused >= 0) select(options[focused]);
      else if (canCreate) select(value.trim());
      else setOpen(false);
    } else if (event.key === "Escape" && open) {
      event.stopPropagation();
      setOpen(false);
      setFocused(-1);
    }
  }

  return (
    <div className="combo" onBlur={() => window.setTimeout(() => setOpen(false), 140)}>
      <input
        className="input"
        placeholder="Ungrouped"
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
          setOpen(true);
          setFocused(-1);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={onKeyDown}
      />
      {open && (options.length > 0 || canCreate) && (
        <div className="combo-list" role="listbox">
          {canCreate && (
            <button
              type="button"
              className="combo-item combo-item-new"
              onMouseDown={(event) => {
                event.preventDefault();
                select(value.trim());
              }}
            >
              Create “{value.trim()}”
            </button>
          )}
          {options.map((group, index) => (
            <button
              type="button"
              key={group}
              role="option"
              aria-selected={index === focused}
              className={`combo-item${index === focused ? " is-focused" : ""}`}
              onMouseDown={(event) => {
                event.preventDefault();
                select(group);
              }}
              onMouseEnter={() => setFocused(index)}
            >
              {group}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/* ── Steps editor ───────────────────────────────────────────────────── */

function StepsEditor({
  steps,
  stepIds,
  onPatch,
  onRemove,
  onAdd,
  onDragEnd,
}: {
  steps: DraftStep[];
  stepIds: string[];
  onPatch: (index: number, patch: Partial<DraftStep>) => void;
  onRemove: (index: number) => void;
  onAdd: () => void;
  onDragEnd: (result: DropResult) => void;
}) {
  return (
    <div className="field">
      <div className="label-row">
        <span className="label">Steps</span>
        <span className="label-hint">Drag to reorder · delay runs after each step</span>
      </div>
      <DragDropContext onDragEnd={onDragEnd}>
        <Droppable droppableId="command-steps">
          {(provided) => (
            <div className="steps" ref={provided.innerRef} {...provided.droppableProps}>
              {steps.map((step, index) => (
                <Draggable key={stepIds[index]} draggableId={stepIds[index]} index={index}>
                  {(dragProvided, snapshot) => (
                    <div
                      className={`step${snapshot.isDragging ? " is-dragging" : ""}`}
                      ref={dragProvided.innerRef}
                      {...dragProvided.draggableProps}
                    >
                      <div className="step-top">
                        <div className="step-handle" {...dragProvided.dragHandleProps} title="Drag to reorder">
                          <IconGrip size={14} />
                        </div>
                        <span className="step-index">{index + 1}</span>
                        <input
                          className="input step-cmd"
                          placeholder="Command to run…"
                          aria-label={`Step ${index + 1} command`}
                          value={step.cmd}
                          onChange={(event) => onPatch(index, { cmd: event.target.value })}
                        />
                        <div className="step-delay">
                          <input
                            type="number"
                            className="input"
                            min={0}
                            max={300}
                            placeholder="0"
                            aria-label={`Step ${index + 1} delay in seconds`}
                            title="Delay in seconds after this step"
                            value={step.delay_sec || ""}
                            onChange={(event) => onPatch(index, { delay_sec: Number(event.target.value) || 0 })}
                          />
                          <span className="step-delay-unit">s</span>
                        </div>
                        <IconButton
                          variant="danger"
                          label={`Remove step ${index + 1}`}
                          disabled={steps.length <= 1}
                          onClick={() => onRemove(index)}
                        >
                          <IconX size={13} />
                        </IconButton>
                      </div>
                      <input
                        className="step-note"
                        placeholder="Step note (optional)"
                        aria-label={`Step ${index + 1} note`}
                        value={step.note ?? ""}
                        onChange={(event) => onPatch(index, { note: event.target.value })}
                      />
                    </div>
                  )}
                </Draggable>
              ))}
              {provided.placeholder}
            </div>
          )}
        </Droppable>
      </DragDropContext>
      <button type="button" className="btn btn-ghost btn-dashed" onClick={onAdd}>
        <IconPlus size={14} />
        Add step
      </button>
    </div>
  );
}

/* ── Command editor modal ────────────────────────────────────────────── */

export default function CommandEditor({
  command,
  terminals,
  groups,
  onClose,
  onSaved,
  onError,
}: {
  command: Command | null;
  terminals: TerminalInfo[];
  groups: string[];
  onClose: () => void;
  onSaved: (message: string) => void;
  onError: (message: string) => void;
}) {
  const initialSteps: DraftStep[] =
    command && command.steps.length > 0
      ? command.steps.map((step) => ({ cmd: step.cmd, delay_sec: step.delay_sec, note: step.note ?? "" }))
      : [{ cmd: command?.command ?? "", delay_sec: 0, note: "" }];

  const [name, setName] = useState(command?.name ?? "");
  const [note, setNote] = useState(command?.note ?? "");
  const [terminal, setTerminal] = useState(command?.terminal ?? terminals[0]?.id ?? "");
  const [group, setGroup] = useState(command?.group_name ?? "");
  const [autoStart, setAutoStart] = useState(command?.auto_start ?? false);
  const [steps, setSteps] = useState<DraftStep[]>(initialSteps);
  const [stepIds, setStepIds] = useState<string[]>(() => initialSteps.map(() => crypto.randomUUID()));
  const [saving, setSaving] = useState(false);

  function patchStep(index: number, patch: Partial<DraftStep>) {
    setSteps((current) => current.map((step, i) => (i === index ? { ...step, ...patch } : step)));
  }

  function addStep() {
    setSteps((current) => [...current, { cmd: "", delay_sec: 0, note: "" }]);
    setStepIds((current) => [...current, crypto.randomUUID()]);
  }

  function removeStep(index: number) {
    if (steps.length <= 1) return;
    setSteps((current) => current.filter((_, i) => i !== index));
    setStepIds((current) => current.filter((_, i) => i !== index));
  }

  function onDragEnd(result: DropResult) {
    const { source, destination } = result;
    if (!destination || destination.index === source.index) return;
    setSteps((current) => {
      const next = [...current];
      const [moved] = next.splice(source.index, 1);
      next.splice(destination.index, 0, moved);
      return next;
    });
    setStepIds((current) => {
      const next = [...current];
      const [moved] = next.splice(source.index, 1);
      next.splice(destination.index, 0, moved);
      return next;
    });
  }

  async function save() {
    const cleanSteps = steps
      .map((step) => ({ cmd: step.cmd.trim(), delay_sec: step.delay_sec || 0, note: (step.note ?? "").trim() }))
      .filter((step) => step.cmd.length > 0);

    if (!name.trim()) {
      onError("Name is required");
      return;
    }
    if (cleanSteps.length === 0) {
      onError("Add at least one command");
      return;
    }
    if (!terminal) {
      onError("Pick a terminal");
      return;
    }

    setSaving(true);
    try {
      if (command) {
        await updateCommand(
          command.id,
          name.trim(),
          cleanSteps[0].cmd,
          terminal,
          autoStart,
          group.trim(),
          cleanSteps,
          note.trim(),
        );
        onSaved("Command updated");
      } else {
        await addCommand(
          name.trim(),
          cleanSteps[0].cmd,
          terminal,
          autoStart,
          group.trim(),
          cleanSteps,
          note.trim(),
        );
        onSaved("Command added");
      }
    } catch (error) {
      onError("Save failed: " + errorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      title={command ? "Edit command" : "New command"}
      subtitle={
        command
          ? "Update what gets launched and how."
          : "Chain one or more commands and pick where they run."
      }
      onClose={onClose}
      dismissOnBackdrop={false}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" form="command-form" className="btn btn-primary" disabled={saving}>
            {saving ? "Saving…" : command ? "Save changes" : "Create command"}
          </button>
        </>
      }
    >
      <form
        id="command-form"
        className="form"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <div className="field">
          <label className="label" htmlFor="cmd-name">
            Name
          </label>
          <input
            id="cmd-name"
            className="input"
            placeholder="e.g. Start dev environment"
            value={name}
            onChange={(event) => setName(event.target.value)}
            autoFocus
          />
        </div>

        <div className="field">
          <label className="label" htmlFor="cmd-note">
            Note <span className="label-hint">optional</span>
          </label>
          <input
            id="cmd-note"
            className="input"
            placeholder="Short description shown in the list"
            value={note}
            onChange={(event) => setNote(event.target.value)}
          />
        </div>

        <div className="field-row">
          <div className="field">
            <label className="label" htmlFor="cmd-terminal">
              Terminal
            </label>
            <select
              id="cmd-terminal"
              className="input select"
              value={terminal}
              onChange={(event) => setTerminal(event.target.value)}
            >
              <option value="" disabled>
                Select…
              </option>
              {terminals.map((option) => (
                <option key={option.id} value={option.id}>
                  {terminalLabel(option)}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <span className="label">Group</span>
            <GroupSelect value={group} groups={groups} onChange={setGroup} />
          </div>
        </div>

        <div className="toggle-row">
          <div className="toggle-row-text">
            <span className="toggle-row-label">Run at startup</span>
            <span className="toggle-row-desc">Launch automatically when OpenStart starts</span>
          </div>
          <Switch
            checked={autoStart}
            label="Run at startup"
            onChange={() => setAutoStart((value) => !value)}
          />
        </div>

        <StepsEditor
          steps={steps}
          stepIds={stepIds}
          onPatch={patchStep}
          onRemove={removeStep}
          onAdd={addStep}
          onDragEnd={onDragEnd}
        />

        {command && (
          <div className="form-meta">
            <span>Created {fmtDateTime(command.created_at)}</span>
            <span>Updated {fmtDateTime(command.updated_at)}</span>
            <span className="form-meta-mono">ID {command.id}</span>
          </div>
        )}
      </form>
    </Modal>
  );
}
