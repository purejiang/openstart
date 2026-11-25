import { useCallback, useEffect, useRef, useState } from "react";
import type { AppInfo, Command, Run, UpdateInfo } from "./types";
import {
  checkUpdate,
  deleteCommand,
  deleteGroup,
  detectTerminals,
  errorMessage,
  executeCommandById,
  executeGroup,
  getAppInfo,
  listCommands,
  listRuns,
  setCommandEnabled,
  stopAllRuns,
  stopGroupRuns,
  stopRun,
} from "./api";
import RunsPanel from "./RunsPanel";
import CommandRow from "./CommandRow";
import CommandEditor from "./CommandEditor";
import SettingsPage from "./SettingsPage";
import { ConfirmDialog, IconButton } from "./ui";
import {
  BrandMark,
  IconAlert,
  IconCheck,
  IconChevronDown,
  IconExternal,
  IconPlay,
  IconPlus,
  IconSearch,
  IconSettings,
  IconStop,
  IconTerminal,
  IconTrash,
  IconX,
} from "./icons";
import "./App.css";

interface Toast {
  id: number;
  message: string;
  error: boolean;
}

export default function App() {
  const [page, setPage] = useState<"commands" | "settings">("commands");
  const [commands, setCommands] = useState<Command[]>([]);
  const [terminals, setTerminals] = useState<Awaited<ReturnType<typeof detectTerminals>>>([]);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [runs, setRuns] = useState<Run[]>([]);
  const [loading, setLoading] = useState(true);
  const [stopping, setStopping] = useState(false);
  const [togglingId, setTogglingId] = useState<string | null>(null);

  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [editor, setEditor] = useState<{ command: Command | null } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Command | null>(null);
  const [deleteGroupTarget, setDeleteGroupTarget] = useState<string | null>(null);

  const [toast, setToast] = useState<Toast | null>(null);
  const toastTimer = useRef<number | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);

  const notify = useCallback((message: string, isError = false) => {
    setToast({ id: Date.now(), message, error: isError });
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 3400);
  }, []);

  useEffect(
    () => () => {
      if (toastTimer.current) window.clearTimeout(toastTimer.current);
    },
    [],
  );

  /* ── Data loading ─────────────────────────────────────────────────── */

  const refresh = useCallback(async () => {
    try {
      const [nextCommands, nextTerminals, info] = await Promise.all([
        listCommands(),
        detectTerminals(),
        getAppInfo(),
      ]);
      setCommands(nextCommands);
      setTerminals(nextTerminals);
      setAppInfo(info);
    } catch (error) {
      notify("Failed to load commands: " + errorMessage(error), true);
    } finally {
      setLoading(false);
    }
  }, [notify]);

  const refreshRuns = useCallback(async () => {
    try {
      setRuns(await listRuns(100));
    } catch {
      /* transient polling failure — keep the last known state */
    }
  }, []);

  const checkForUpdates = useCallback(
    async (manual: boolean) => {
      try {
        const info = await checkUpdate();
        if (info.update_available) setUpdateInfo(info);
        else if (manual) notify("You're on the latest version");
      } catch (error) {
        if (manual) notify("Update check failed: " + errorMessage(error), true);
      }
    },
    [notify],
  );

  useEffect(() => {
    void refresh().then(() => checkForUpdates(false));
  }, [refresh, checkForUpdates]);

  // Keep the live-run view honest without reloading the whole command list.
  useEffect(() => {
    let alive = true;
    const tick = async () => {
      if (document.hidden) return;
      try {
        const next = await listRuns(100);
        if (alive) setRuns(next);
      } catch {
        /* ignore transient failures */
      }
    };
    void tick();
    const timer = window.setInterval(tick, 2500);
    const onVisibility = () => {
      if (!document.hidden) void tick();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      alive = false;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);

  /* ── Command actions ──────────────────────────────────────────────── */

  async function handleExec(command: Command, groupName: string) {
    if (!command.enabled) {
      notify("This command is disabled. Enable it before running.", true);
      return;
    }
    try {
      const useExistingWindow = command.terminal.startsWith("terminal");
      const run = await executeCommandById(
        command.id,
        useExistingWindow,
        useExistingWindow ? groupName || command.group_name : undefined,
      );
      setRuns((current) => [run, ...current.filter((item) => item.id !== run.id)]);
      notify(`Started ${command.name}${run.pid ? ` · PID ${run.pid}` : ""}`);
      await refresh();
    } catch (error) {
      notify("Failed: " + errorMessage(error), true);
    } finally {
      await refreshRuns();
    }
  }

  async function handleToggleEnabled(command: Command) {
    setTogglingId(command.id);
    try {
      const enabled = !command.enabled;
      await setCommandEnabled(command.id, enabled);
      notify(`${command.name} ${enabled ? "enabled" : "disabled"}`);
      await refresh();
    } catch (error) {
      notify("Update failed: " + errorMessage(error), true);
    } finally {
      setTogglingId(null);
    }
  }

  async function handleRunGroup(groupName: string) {
    if (!commands.some((command) => (command.group_name || "") === groupName && command.enabled)) {
      notify("No enabled commands in this group", true);
      return;
    }
    try {
      const started = await executeGroup(groupName);
      if (started.length > 0) {
        setRuns((current) => [...started, ...current.filter((run) => !started.some((item) => item.id === run.id))]);
      }
      const failed = started.filter((run) => run.status === "failed");
      if (failed.length > 0) {
        notify(`Group started with ${failed.length} failure(s)`, true);
      } else {
        notify(`Running ${started.length} command(s)`);
      }
      await refresh();
    } catch (error) {
      notify("Group run failed: " + errorMessage(error), true);
    } finally {
      await refreshRuns();
    }
  }

  async function handleStopRun(runId: string) {
    setStopping(true);
    try {
      await stopRun(runId);
      notify("Stopped");
    } catch (error) {
      notify("Stop failed: " + errorMessage(error), true);
    } finally {
      setStopping(false);
      await refreshRuns();
    }
  }

  async function handleStopGroup(groupName: string) {
    setStopping(true);
    try {
      const summary = await stopGroupRuns(groupName);
      if (summary.failed.length > 0) notify(`Stopped ${summary.stopped}, ${summary.failed.length} failed`, true);
      else notify(`Stopped ${summary.stopped} run(s)`);
    } catch (error) {
      notify("Group stop failed: " + errorMessage(error), true);
    } finally {
      setStopping(false);
      await refreshRuns();
    }
  }

  async function handleStopAll() {
    setStopping(true);
    try {
      const summary = await stopAllRuns();
      if (summary.failed.length > 0) notify(`Stopped ${summary.stopped}, ${summary.failed.length} failed`, true);
      else notify(`Stopped ${summary.stopped} run(s)`);
    } catch (error) {
      notify("Stop all failed: " + errorMessage(error), true);
    } finally {
      setStopping(false);
      await refreshRuns();
    }
  }

  async function confirmDeleteCommand() {
    if (!deleteTarget) return;
    try {
      await deleteCommand(deleteTarget.id);
      notify("Command deleted");
      setDeleteTarget(null);
      await refresh();
    } catch (error) {
      notify("Delete failed: " + errorMessage(error), true);
    }
  }

  async function confirmDeleteGroup() {
    if (deleteGroupTarget === null) return;
    try {
      await deleteGroup(deleteGroupTarget);
      notify("Group deleted");
      setDeleteGroupTarget(null);
      await refresh();
    } catch (error) {
      notify("Delete group failed: " + errorMessage(error), true);
    }
  }

  function toggleSet(set: Set<string>, value: string): Set<string> {
    const next = new Set(set);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    return next;
  }

  /* ── Derived view model ───────────────────────────────────────────── */

  const activeRuns = runs.filter((run) => run.status === "starting" || run.status === "running");
  const activeByCommand = new Map<string, Run>();
  const activeByGroup = new Map<string, Run[]>();
  const lastByCommand = new Map<string, Run>();
  for (const run of activeRuns) {
    if (run.target_kind === "command" && !activeByCommand.has(run.target_id)) {
      activeByCommand.set(run.target_id, run);
    }
    if (run.target_kind !== "command") continue;
    const key = run.group_name || "";
    const list = activeByGroup.get(key);
    if (list) list.push(run);
    else activeByGroup.set(key, [run]);
  }
  for (const run of runs) {
    if (run.target_kind === "command" && !lastByCommand.has(run.target_id)) {
      lastByCommand.set(run.target_id, run);
    }
  }

  const groupMap = new Map<string, Command[]>();
  for (const command of commands) {
    const key = command.group_name || "";
    const list = groupMap.get(key);
    if (list) list.push(command);
    else groupMap.set(key, [command]);
  }

  const needle = query.trim().toLowerCase();
  const matchesQuery = (command: Command) =>
    needle.length === 0 ||
    [command.name, command.note, command.group_name, command.command, ...command.steps.map((step) => step.cmd)]
      .join(" ")
      .toLowerCase()
      .includes(needle);
  const visibleGroups = Array.from(groupMap.entries())
    .map(([group, list]) => ({ group, list: list.filter(matchesQuery) }))
    .filter((entry) => entry.list.length > 0);
  const matchCount = visibleGroups.reduce((total, entry) => total + entry.list.length, 0);

  const disabledCount = commands.filter((command) => !command.enabled).length;
  const existingGroups = Array.from(groupMap.keys()).filter((group) => group);

  /* ── Commands page ────────────────────────────────────────────────── */

  const commandsPage = (
    <div className="page">
      <header className="page-head">
        <div className="page-head-main">
          <h1 className="page-title">Commands</h1>
          {commands.length > 0 && (
            <p className="page-sub">
              <span>
                {commands.length} command{commands.length === 1 ? "" : "s"}
              </span>
              <span className="sep" />
              <span>
                {groupMap.size} group{groupMap.size === 1 ? "" : "s"}
              </span>
              {disabledCount > 0 && (
                <>
                  <span className="sep" />
                  <span>{disabledCount} disabled</span>
                </>
              )}
              {needle.length > 0 && (
                <>
                  <span className="sep" />
                  <span>
                    {matchCount} match{matchCount === 1 ? "" : "es"}
                  </span>
                </>
              )}
            </p>
          )}
        </div>
        {commands.length > 0 && (
          <div className="page-head-actions">
            <label className="search">
              <IconSearch size={14} />
              <input
                type="search"
                value={query}
                placeholder="Search commands…"
                aria-label="Search commands"
                onChange={(event) => setQuery(event.target.value)}
              />
              {query.length > 0 && (
                <button
                  type="button"
                  className="search-clear"
                  aria-label="Clear search"
                  onClick={() => setQuery("")}
                >
                  <IconX size={12} />
                </button>
              )}
            </label>
            <button className="btn btn-primary" onClick={() => setEditor({ command: null })}>
              <IconPlus size={15} />
              New command
            </button>
          </div>
        )}
      </header>

      <RunsPanel runs={runs} stopping={stopping} onStop={handleStopRun} onStopAll={handleStopAll} />

      {loading ? (
        <div className="skeleton-list" aria-busy="true" aria-label="Loading commands">
          {[0, 1].map((index) => (
            <div className="skeleton-group" key={index}>
              <div className="skeleton skeleton-head" />
              <div className="skeleton skeleton-row" />
              <div className="skeleton skeleton-row" />
            </div>
          ))}
        </div>
      ) : commands.length === 0 ? (
        <div className="empty">
          <div className="empty-icon">
            <IconTerminal size={20} />
          </div>
          <h2 className="empty-title">No commands yet</h2>
          <p className="empty-desc">
            Save the terminals, scripts and dev servers you open every day — then launch them in one click, or
            automatically at login.
          </p>
          <button className="btn btn-primary" onClick={() => setEditor({ command: null })}>
            <IconPlus size={15} />
            Create your first command
          </button>
        </div>
      ) : visibleGroups.length === 0 ? (
        <div className="empty empty-compact">
          <h2 className="empty-title">No results</h2>
          <p className="empty-desc">
            Nothing matches “{query.trim()}”. Try a different name, group or command.
          </p>
          <button className="btn btn-ghost" onClick={() => setQuery("")}>
            Clear search
          </button>
        </div>
      ) : (
        visibleGroups.map(({ group, list }) => {
          const fullList = groupMap.get(group) ?? list;
          const enabledCount = fullList.filter((command) => command.enabled).length;
          const activeCount = activeByGroup.get(group)?.length ?? 0;
          // A search should always reveal matches, even inside a collapsed group.
          const isCollapsed = needle.length === 0 && collapsed.has(group);
          return (
            <section className="group" key={group || "__ungrouped__"}>
              <div className="group-head">
                <button
                  type="button"
                  className={`group-chevron${isCollapsed ? " is-collapsed" : ""}`}
                  aria-expanded={!isCollapsed}
                  aria-label={`${isCollapsed ? "Expand" : "Collapse"} ${group || "Ungrouped"}`}
                  onClick={() => setCollapsed((current) => toggleSet(current, group))}
                >
                  <IconChevronDown size={14} />
                </button>
                <h2 className="group-title">{group || "Ungrouped"}</h2>
                <span className="group-meta">
                  {list.length} command{list.length === 1 ? "" : "s"}
                </span>
                {enabledCount < list.length && (
                  <span className="group-meta">{list.length - enabledCount} disabled</span>
                )}
                <div className="group-actions">
                  {activeCount > 0 && (
                    <span className="group-live">
                      <span className="run-dot running" />
                      {activeCount} running
                    </span>
                  )}
                  {activeCount > 0 ? (
                    <button
                      className="btn btn-sm btn-danger-ghost"
                      onClick={() => handleStopGroup(group)}
                      disabled={stopping}
                    >
                      <IconStop size={11} />
                      Stop
                    </button>
                  ) : (
                    <button
                      className="btn btn-sm btn-ghost"
                      onClick={() => handleRunGroup(group)}
                      disabled={stopping || enabledCount === 0}
                      title={
                        enabledCount === 0
                          ? "No enabled commands in this group"
                          : `Run ${enabledCount} enabled command(s)`
                      }
                    >
                      <IconPlay size={11} />
                      Run
                    </button>
                  )}
                  <IconButton
                    variant="danger"
                    className="group-delete"
                    label={`Delete group ${group || "Ungrouped"}`}
                    onClick={() => setDeleteGroupTarget(group)}
                  >
                    <IconTrash size={14} />
                  </IconButton>
                </div>
              </div>
              {!isCollapsed && (
                <div className="cmd-list">
                  {list.map((command) => (
                    <CommandRow
                      key={command.id}
                      command={command}
                      terminals={terminals}
                      activeRun={activeByCommand.get(command.id)}
                      lastRun={lastByCommand.get(command.id)}
                      expanded={expanded.has(command.id)}
                      stopping={stopping}
                      toggling={togglingId === command.id}
                      onToggleEnabled={(value) => void handleToggleEnabled(value)}
                      onRun={(value) => void handleExec(value, group)}
                      onStop={(runId) => void handleStopRun(runId)}
                      onEdit={(value) => setEditor({ command: value })}
                      onDelete={setDeleteTarget}
                      onToggleSteps={(id) => setExpanded((current) => toggleSet(current, id))}
                    />
                  ))}
                </div>
              )}
            </section>
          );
        })
      )}
    </div>
  );

  return (
    <>
      <header className="appbar">
        <div className="brand">
          <BrandMark size={24} />
          <span className="brand-title">OpenStart</span>
        </div>
        <nav className="tabs" aria-label="Main navigation">
          <button
            type="button"
            className={`tab${page === "commands" ? " is-active" : ""}`}
            aria-current={page === "commands" ? "page" : undefined}
            onClick={() => setPage("commands")}
          >
            <IconTerminal size={14} />
            Commands
          </button>
          <button
            type="button"
            className={`tab${page === "settings" ? " is-active" : ""}`}
            aria-current={page === "settings" ? "page" : undefined}
            onClick={() => setPage("settings")}
          >
            <IconSettings size={14} />
            Settings
          </button>
        </nav>
      </header>

      <main className="main">
        {page === "commands" ? (
          commandsPage
        ) : (
          <SettingsPage
            appInfo={appInfo}
            onToast={notify}
            onCheckUpdate={() => checkForUpdates(true)}
          />
        )}
      </main>

      {editor && (
        <CommandEditor
          command={editor.command}
          terminals={terminals}
          groups={existingGroups}
          onClose={() => setEditor(null)}
          onSaved={(message) => {
            setEditor(null);
            notify(message);
            void refresh();
          }}
          onError={(message) => notify(message, true)}
        />
      )}

      {deleteTarget && (
        <ConfirmDialog
          title="Delete command"
          message={
            <>
              Delete <strong>{deleteTarget.name}</strong>? Running terminals are left untouched, but the command
              is removed from startup and from this list.
            </>
          }
          onCancel={() => setDeleteTarget(null)}
          onConfirm={() => void confirmDeleteCommand()}
        />
      )}

      {deleteGroupTarget !== null && (
        <ConfirmDialog
          title="Delete group"
          message={
            <>
              Delete <strong>{deleteGroupTarget || "Ungrouped"}</strong> and its{" "}
              <strong>{groupMap.get(deleteGroupTarget)?.length ?? 0}</strong> command(s)? This cannot be undone.
            </>
          }
          confirmLabel="Delete group"
          onCancel={() => setDeleteGroupTarget(null)}
          onConfirm={() => void confirmDeleteGroup()}
        />
      )}

      <div className="toasts" aria-live="polite">
        {updateInfo?.update_available && (
          <div className="toast toast-update">
            <span className="toast-icon">
              <IconAlert size={14} />
            </span>
            <span>
              Update available · <strong>v{updateInfo.latest_version}</strong>
            </span>
            <a className="toast-link" href={updateInfo.download_url} target="_blank" rel="noreferrer">
              Download
              <IconExternal size={11} />
            </a>
            <IconButton label="Dismiss update notice" onClick={() => setUpdateInfo(null)}>
              <IconX size={12} />
            </IconButton>
          </div>
        )}
        {toast && (
          <div className={`toast${toast.error ? " toast-error" : ""}`} role="status" key={toast.id}>
            <span className="toast-icon">
              {toast.error ? <IconAlert size={14} /> : <IconCheck size={14} />}
            </span>
            {toast.message}
          </div>
        )}
      </div>
    </>
  );
}
