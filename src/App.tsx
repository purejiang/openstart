import { useCallback, useEffect, useState } from "react";
import type { Command, CommandStep, TerminalInfo, AppSettings, AppInfo, UpdateInfo } from "./types";
import {
  addCommand, checkUpdate, deleteCommand, deleteGroup, detectTerminals, executeCommandById, executeGroup,
  getAppInfo, getSettings, listCommands, readLogs, updateCommand, updateSettings,
} from "./api";
import "./App.css";

const ICONS = {
  plus: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>),
  play: (<svg viewBox="0 0 24 24" fill="currentColor" stroke="none"><path d="M8 5.14v14l11-7-11-7z" /></svg>),
  playAll: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polygon points="5 3 19 12 5 21 5 3" /><line x1="10" y1="3" x2="10" y2="21" /></svg>),
  edit: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>),
  trash: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>),
  close: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>),
  zap: (<svg viewBox="0 0 1024 1024" width="24" height="24"><path d="M0 0m240 0l544 0q240 0 240 240l0 544q0 240-240 240l-544 0q-240 0-240-240l0-544q0-240 240-240Z" fill="#242938"/><path d="M815.276 275.34l-252.56-149.92a95.16 95.16 0 0 0-97.444 0l-252.56 149.92C182.568 293.24 164 326.3 164 362.088v299.844c0 35.78 18.572 68.86 48.72 86.756l252.56 149.892a95.2 95.2 0 0 0 48.716 13.416 95.2 95.2 0 0 0 48.712-13.416l252.56-149.892c30.144-17.896 48.728-50.976 48.728-86.756v-299.84c0-35.792-18.584-68.856-48.72-86.752" fill="#242938"/><path d="M474.108 883.232l-252.56-149.896c-24.704-14.664-40.052-42.024-40.052-71.408V362.092c0-29.384 15.348-56.744 40.04-71.4l252.572-149.92a78.2 78.2 0 0 1 39.888-10.988c13.98 0 27.772 3.8 39.892 10.988l252.56 149.92c20.816 12.356 34.856 33.752 38.804 57.748-8.376-17.876-27.268-22.736-49.28-9.88l-238.936 147.588c-29.792 17.416-51.76 36.96-51.78 72.884v294.416c-0.016 21.512 8.672 35.444 22.016 39.484-4.384 0.76-8.804 1.288-13.276 1.288a78.2 78.2 0 0 1-39.888-10.988m341.168-607.896l-252.56-149.912A95.2 95.2 0 0 0 513.996 112a95.2 95.2 0 0 0-48.724 13.424l-252.56 149.912C182.568 293.232 164 326.304 164 362.096v299.832c0 35.78 18.572 68.86 48.72 86.756l252.56 149.9A95.36 95.36 0 0 0 513.996 912a95.32 95.32 0 0 0 48.712-13.416l252.568-149.9c30.144-17.896 48.72-50.976 48.72-86.756V362.092c0-35.788-18.576-68.86-48.72-86.756" fill="#FFFFFF"/><path d="M749.068 690.916l-62.888 37.64c-1.668 0.972-2.892 2.064-2.904 4.068v16.456c0 2.012 1.352 2.848 3.016 1.868l63.864-38.812c1.664-0.972 1.92-2.832 1.932-4.836v-14.516c0-2-1.352-2.84-3.02-1.868" fill="#47B353"/><path d="M615.152 552.392c2.036-1.032 3.712 0.236 3.74 2.9l0.212 21.092c0.024 2.668-1.332 4.168-2.992 4.168H387.26c-1.668 0-2.968-1.204-2.968-2.904v-22.34c0-2.664 1.348-4.088 3.016-2.9l19.296 14.16 17.172-13.196 16.156 13.196 17.72-13.196 17.16 13.196 17.16-13.196 17.744 13.196 16.16-13.196 17.72 13.196 16.168-13.196 17.72 13.196 16.176-13.196 17.72 13.196 16.172-13.196 17.712 13.196 16.18-13.196 17.72 13.196 16.18-13.196 17.72 13.196 16.16-13.196 17.72 13.196 16.18-13.196 17.72 13.196 16.18-13.196 17.72 13.196 16.17-13.196 17.71 13.196 16.18-13.196Z" fill="#47B353"/><path d="M378.376 789.322l238.828-147.448c30.148-18.624 48.72-52.312 48.72-88.664V440.418c0-21.812-12.56-34.148-28.472-27.732-10.936 3.988-29.368 18.032-29.42 29.156-0.052 13.12-2.96 178.184-1.712 244.712 1.252 66.528-9.424 57.944-13.788 51.92-4.356-6.024-12.504-19.028-29.736-13.184-17.228 5.852-13.512 22.612-13.588 29.956 0 7.344-18.428 10.156-40.576 15.772s-43.796 14.456-44.396 28.072c-0.6 13.62 24.18 14.932 35 10.58 10.828-4.352 30.768-11.528 49.608-15.448 18.836-3.924 34.092-2.82 48.844 4.648 14.764 7.468 30.668 1.372 30.704-8.548 0.032-9.924 6.48-29.716-24.68-18.568" fill="#FFFFFF"/></svg>),
  terminal: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" /></svg>),
  gear: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3" /><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" /></svg>),
  chevron: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><polyline points="6 9 12 15 18 9" /></svg>),
} as const;

function terminalClass(id: string): string {
  const m: Record<string, string> = { powershell: "term-powershell", cmd: "term-cmd", gitbash: "term-gitbash", terminal: "term-winterm" };
  return m[id] ?? "term-default";
}
function terminalLabel(term: TerminalInfo): string {
  const m: Record<string, string> = { powershell: "PowerShell", cmd: "CMD", gitbash: "Git Bash", terminal: "Windows Terminal" };
  return m[term.id] ?? term.name;
}

/* ── Group Selector ──────────────────────────────── */
function GroupSelect({ value, groups, onChange }: { value: string; groups: string[]; onChange: (v: string) => void }) {
  const [open, setOpen] = useState(false);
  const [focused, setFocused] = useState(-1);
  const filtered = groups.filter(g => g.toLowerCase().includes(value.toLowerCase()));
  const show = open && (filtered.length > 0 || value);

  function select(g: string) { onChange(g); setOpen(false); setFocused(-1); }
  function onKey(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") { e.preventDefault(); setFocused(f => Math.min(f + 1, filtered.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setFocused(f => Math.max(f - 1, -1)); }
    else if (e.key === "Enter" && focused >= 0) { select(filtered[focused]); }
    else if (e.key === "Escape") { setOpen(false); setFocused(-1); }
  }

  return (
    <div className="group-select" onBlur={() => setTimeout(() => setOpen(false), 150)}>
      <input
        className="form-input"
        placeholder="Pick or type new group"
        value={value}
        onChange={e => onChange(e.target.value)}
        onFocus={() => setOpen(true)}
        onKeyDown={onKey}
      />
      {show && (
        <div className="group-dropdown">
          {value && !groups.includes(value) && (
            <div className="group-dropdown-item group-dropdown-new" onMouseDown={() => select(value)}>
              Create "{value}"
            </div>
          )}
          {filtered.map((g, i) => (
            <div
              key={g}
              className={`group-dropdown-item${i === focused ? " focused" : ""}`}
              onMouseDown={() => select(g)}
              onMouseEnter={() => setFocused(i)}
            >
              {g}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

interface CmdForm { name: string; steps: CommandStep[]; terminal: string; auto_start: boolean; group_name: string; }
const EMPTY: CmdForm = { name: "", steps: [{ cmd: "", delay_sec: 0 }], terminal: "", auto_start: false, group_name: "" };

export default function App() {
  const [page, setPage] = useState<"commands" | "settings">("commands");
  const [commands, setCommands] = useState<Command[]>([]);
  const [terminals, setTerminals] = useState<TerminalInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form, setForm] = useState<CmdForm>(EMPTY);
  const [delId, setDelId] = useState<string | null>(null);
  const [delGroup, setDelGroup] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  function toggleGroup(g: string) {
    setCollapsed(prev => {
      const next = new Set(prev);
      if (next.has(g)) next.delete(g); else next.add(g);
      return next;
    });
  }
  const [appSettings, setAppSettings] = useState<AppSettings>({ app_autostart: false, startup_delay_seconds: 3, close_to_tray: true, keep_terminal_open: true });
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [dirty, setDirty] = useState(false);
  const [toast, setToast] = useState<{ message: string; error?: boolean } | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [showLogs, setShowLogs] = useState(false);
  const [logContent, setLogContent] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [cmds, terms, s, info] = await Promise.all([listCommands(), detectTerminals(), getSettings(), getAppInfo()]);
      setCommands(cmds); setTerminals(terms); setAppSettings(s); setAppInfo(info);
    } catch (e) { toastMsg("Failed to load: " + String(e), true); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { refresh().then(() => { checkForUpdates(); }); }, [refresh]);

  function toastMsg(m: string, e?: boolean) { setToast({ message: m, error: e }); setTimeout(() => setToast(null), 3000); }

  async function checkForUpdates() {
    try {
      const info = await checkUpdate();
      if (info.update_available) {
        setUpdateInfo(info);
      } else {
        toastMsg("You're up to date!");
      }
    } catch (e) {
      toastMsg("Update check failed: " + String(e), true);
    }
  }

  function openAdd() {
    const t = terminals.length > 0 ? terminals[0].id : "";
    setForm({ name: "", steps: [{ cmd: "", delay_sec: 0 }], terminal: t, auto_start: false, group_name: "" });
    setEditingId(null); setShowForm(true);
  }
  function openEdit(c: Command) {
    const steps = c.steps.length > 0 ? [...c.steps] : [{ cmd: c.command, delay_sec: 0 }];
    setForm({ name: c.name, steps, terminal: c.terminal, auto_start: c.auto_start, group_name: c.group_name });
    setEditingId(c.id); setShowForm(true);
  }
  function closeForm() { setShowForm(false); setEditingId(null); }

  async function handleSave() {
    const { name, steps, terminal, auto_start, group_name } = form;
    if (!name.trim()) { toastMsg("Name required", true); return; }
    const validSteps = steps.filter(s => s.cmd.trim());
    if (validSteps.length === 0) { toastMsg("At least one command required", true); return; }
    if (!terminal) { toastMsg("Terminal required", true); return; }
    try {
      const cmd = validSteps[0].cmd.trim();
      if (editingId) {
        await updateCommand(editingId, name.trim(), cmd, terminal, auto_start, group_name.trim(), validSteps);
      } else {
        await addCommand(name.trim(), cmd, terminal, auto_start, group_name.trim(), validSteps);
      }
      closeForm(); await refresh();
    } catch (e) { toastMsg("Save failed: " + String(e), true); }
  }

  async function handleExec(c: Command, groupName?: string) {
    try {
      const useExisting = c.terminal.startsWith("terminal");
      await executeCommandById(c.id, useExisting, useExisting ? (groupName || c.group_name) : undefined);
      toastMsg(`Executed: ${c.name}`);
    }
    catch (e) { toastMsg("Failed: " + String(e), true); }
  }
  async function handleRunGroup(g: string) {
    try { await executeGroup(g); toastMsg(`Running group: ${g || "default"}`); }
    catch (e) { toastMsg("Group run failed: " + String(e), true); }
  }
  async function handleDelete() {
    if (!delId) return;
    try { await deleteCommand(delId); toastMsg("Deleted"); setDelId(null); await refresh(); }
    catch (e) { toastMsg("Delete failed: " + String(e), true); }
  }
  async function handleDeleteGroup() {
    if (!delGroup) return;
    try { await deleteGroup(delGroup); toastMsg(`Group "${delGroup}" deleted`); setDelGroup(null); await refresh(); }
    catch (e) { toastMsg("Delete group failed: " + String(e), true); }
  }
  async function handleSaveSettings() {
    try { await updateSettings(appSettings.app_autostart, appSettings.startup_delay_seconds, appSettings.close_to_tray, appSettings.keep_terminal_open); toastMsg("Saved"); setDirty(false); }
    catch (e) { toastMsg("Save failed: " + String(e), true); }
  }

  /* ── Group commands ──────────────────────────────── */
  const groups = new Map<string, Command[]>();
  for (const c of commands) { const g = c.group_name || ""; if (!groups.has(g)) groups.set(g, []); groups.get(g)!.push(c); }
  const existingGroups = Array.from(new Set(commands.map(c => c.group_name).filter(g => g)));

  const cmdPage = (
    <>
      {loading ? (<div className="loading-state"><div className="loading-spinner" />Loading…</div>)
      : commands.length === 0 ? (
        <div className="empty-state">
          <div className="empty-icon">{ICONS.terminal}</div>
          <div className="empty-title">No commands yet</div>
          <button className="btn-primary" onClick={openAdd}>{ICONS.plus}Add Command</button>
        </div>
      ) : (
        <>
          <div className="toolbar">
            <span className="toolbar-info">{commands.length} commands · {groups.size} groups</span>
            <div className="toolbar-spacer" />
            <button className="btn-add-icon" onClick={openAdd} title="Add">{ICONS.plus}</button>
          </div>
          {Array.from(groups.entries()).map(([gname, gcmds]) => {
            const allWt = gcmds.every(c => c.terminal.startsWith("terminal"));
            return (
              <div key={gname || "__default__"} className="group-block">
                <div className="group-header">
                  <button className={`group-chevron${collapsed.has(gname) ? " collapsed" : ""}`} onClick={() => toggleGroup(gname)}>{ICONS.chevron}</button>
                  <span className="group-name">{gname || "Default"}</span>
                  <span className="group-meta">{gcmds.length} cmd{allWt ? " · tabs" : " · separate"}</span>
                  <button className="btn-group-run" onClick={() => handleRunGroup(gname)}>{ICONS.playAll} Run</button>
                  <button className="btn-icon btn-icon-delete" title="Delete group" onClick={() => setDelGroup(gname)}>{ICONS.trash}</button>
                </div>
                {!collapsed.has(gname) && (
                <div className="cmd-card-list">
                  {gcmds.map(c => {
                    const t = terminals.find(x => x.id === c.terminal) ?? ({ id: c.terminal, name: c.terminal } as TerminalInfo);
                    return (
                      <div className="cmd-card" key={c.id}>
                        <div className="cmd-card-header">
                          <span className={`auto-start-dot${c.auto_start ? " active" : ""}`} title={c.auto_start ? "Auto-start enabled" : "Auto-start disabled"} />
                          <span className="cmd-card-name" title={c.name}>{c.name}</span>
                          <div className="cmd-card-spacer" />
                          <div className="cmd-card-meta">
                            <span className={`terminal-badge ${terminalClass(c.terminal)}`} title={terminalLabel(t)}>
                              {terminalLabel(t)}
                            </span>
                            <div className="row-actions">
                              <button className="btn-icon btn-icon-execute" title="Execute" onClick={() => handleExec(c, gname)}>{ICONS.play}</button>
                              <button className="btn-icon btn-icon-edit" title="Edit" onClick={() => openEdit(c)}>{ICONS.edit}</button>
                              <button className="btn-icon btn-icon-delete" title="Delete" onClick={() => setDelId(c.id)}>{ICONS.trash}</button>
                            </div>
                          </div>
                        </div>
                        <div className="cmd-card-command" title={c.steps.length > 0 ? c.steps.map(s => s.cmd).join(" → ") : c.command}>
                          {c.steps.length > 0 ? c.steps[0].cmd : c.command}
                          {c.steps.length > 1 && <span className="step-badge">{c.steps.length} steps</span>}
                        </div>
                      </div>
                    );
                  })}
                </div>
                )}
              </div>
            );
          })}
        </>
      )}
    </>
  );

  const setPage2 = (
    <div className="settings-page">
      <section className="settings-section">
        <h3 className="settings-section-title">About</h3>
        <p className="settings-section-subtitle">Application information and updates</p>
        <div className="settings-divider" />
        {appInfo && (<div className="settings-info-grid"><span className="info-label">Name</span><span className="info-value">{appInfo.name}</span><span className="info-label">Version</span><span className="info-value">{appInfo.version}</span><span className="info-label">Data</span><span className="info-value info-value-mono">{appInfo.dataDir}</span></div>)}
        <div className="settings-actions">
          <button className="btn-update" onClick={checkForUpdates}>{ICONS.zap}Check for Updates</button>
          <button className="btn-ghost" onClick={async () => {
            try {
              const logs = await readLogs(200);
              setLogContent(logs);
              setShowLogs(true);
            } catch (e) { toastMsg("Failed to read logs: " + String(e), true); }
          }}>{ICONS.terminal}View Logs</button>
        </div>
      </section>
      <section className="settings-section">
        <h3 className="settings-section-title">Startup</h3>
        <p className="settings-section-subtitle">Configure launch and terminal behavior</p>
        <div className="settings-divider" />
        <div className="settings-row">
          <div className="settings-row-text"><span className="settings-row-label">Launch at system startup</span><span className="settings-row-desc">Run OpenStart automatically when Windows starts</span></div>
          <button className={`toggle${appSettings.app_autostart ? " active" : ""}`} onClick={() => { setAppSettings(s => ({ ...s, app_autostart: !s.app_autostart })); setDirty(true); }}><span className="toggle-knob" /></button>
        </div>
        <div className="settings-row">
          <div className="settings-row-text"><span className="settings-row-label">Minimize to tray on close</span><span className="settings-row-desc">Closing the window hides to tray instead of quitting</span></div>
          <button className={`toggle${appSettings.close_to_tray ? " active" : ""}`} onClick={() => { setAppSettings(s => ({ ...s, close_to_tray: !s.close_to_tray })); setDirty(true); }}><span className="toggle-knob" /></button>
        </div>
        <div className="settings-row">
          <div className="settings-row-text"><span className="settings-row-label">Keep terminal open after execution</span><span className="settings-row-desc">Terminal stays open when the command finishes</span></div>
          <button className={`toggle${appSettings.keep_terminal_open ? " active" : ""}`} onClick={() => { setAppSettings(s => ({ ...s, keep_terminal_open: !s.keep_terminal_open })); setDirty(true); }}><span className="toggle-knob" /></button>
        </div>
        <div className="settings-row">
          <div className="settings-row-text"><span className="settings-row-label">Startup delay</span><span className="settings-row-desc">Seconds to wait before auto-running commands</span></div>
          <input type="number" className="settings-input-num" min={0} max={60} value={appSettings.startup_delay_seconds} onChange={e => { setAppSettings(s => ({ ...s, startup_delay_seconds: Number(e.target.value) || 0 })); setDirty(true); }} />
        </div>
      </section>
      {dirty && (
        <div className="settings-savebar">
          <span className="settings-savebar-text">You have unsaved changes</span>
          <button className="btn-save" onClick={handleSaveSettings}>Save Settings</button>
        </div>
      )}
    </div>
  );

  return (
    <>
      <header className="navbar">
        <div className="navbar-left"><div className="navbar-logo">{ICONS.zap}</div><span className="navbar-title">OpenStart</span></div>
        <div className="navbar-right">
          <button className={`nav-btn${page === "commands" ? " active" : ""}`} onClick={() => setPage("commands")}>{ICONS.terminal} Commands</button>
          <button className={`nav-btn${page === "settings" ? " active" : ""}`} onClick={() => setPage("settings")}>{ICONS.gear} Settings</button>
        </div>
      </header>
      <main className="main-content">{page === "commands" ? cmdPage : setPage2}</main>

      {showForm && (<div className="modal-overlay"><div className="modal" onClick={e => e.stopPropagation()}>
        <div className="modal-header"><span className="modal-title">{editingId ? "Edit" : "Add"} Command</span><button className="modal-close" onClick={closeForm}>{ICONS.close}</button></div>
        <div className="modal-body">
          <div className="form-group"><label className="form-label" htmlFor="f-name">Name</label><input id="f-name" className="form-input" placeholder="e.g. Start dev" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} autoFocus /></div>
          <div className="form-group">
            <label className="form-label">Steps</label>
            <div className="steps-editor">
              {form.steps.map((step, i) => (
                <div key={i} className="step-row">
                  <span className="step-number">{i + 1}</span>
                  <input
                    className="form-input step-cmd-input"
                    placeholder="Command..."
                    value={step.cmd}
                    onChange={e => {
                      const s = [...form.steps];
                      s[i] = { ...s[i], cmd: e.target.value };
                      setForm(f => ({ ...f, steps: s }));
                    }}
                  />
                  <input
                    type="number"
                    className="form-input step-delay-input"
                    min={0}
                    max={300}
                    placeholder="Delay"
                    title="Delay in seconds after this step"
                    value={step.delay_sec || ""}
                    onChange={e => {
                      const s = [...form.steps];
                      s[i] = { ...s[i], delay_sec: Number(e.target.value) || 0 };
                      setForm(f => ({ ...f, steps: s }));
                    }}
                  />
                  <button className="step-btn" title="Move up" disabled={i === 0} onClick={() => {
                    if (i === 0) return;
                    const s = [...form.steps];
                    [s[i - 1], s[i]] = [s[i], s[i - 1]];
                    setForm(f => ({ ...f, steps: s }));
                  }}>↑</button>
                  <button className="step-btn" title="Move down" disabled={i === form.steps.length - 1} onClick={() => {
                    if (i === form.steps.length - 1) return;
                    const s = [...form.steps];
                    [s[i], s[i + 1]] = [s[i + 1], s[i]];
                    setForm(f => ({ ...f, steps: s }));
                  }}>↓</button>
                  <button className="step-btn step-btn-remove" title="Remove" disabled={form.steps.length <= 1} onClick={() => {
                    const s = form.steps.filter((_, j) => j !== i);
                    setForm(f => ({ ...f, steps: s.length > 0 ? s : [{ cmd: "", delay_sec: 0 }] }));
                  }}>✕</button>
                </div>
              ))}
              <button className="step-add-btn" onClick={() => {
                setForm(f => ({ ...f, steps: [...f.steps, { cmd: "", delay_sec: 0 }] }));
              }}>+ Add Step</button>
            </div>
          </div>
          <div className="form-group"><label className="form-label" htmlFor="f-term">Terminal</label><select id="f-term" className="form-select" value={form.terminal} onChange={e => setForm(f => ({ ...f, terminal: e.target.value }))}><option value="" disabled>Select…</option>{terminals.map(t => (<option key={t.id} value={t.id}>{terminalLabel(t)}</option>))}</select></div>
          <div className="form-group"><label className="form-label" htmlFor="f-grp">Group</label>
                <GroupSelect value={form.group_name} groups={existingGroups} onChange={v => setForm(f => ({ ...f, group_name: v }))} />
              </div>
          <div className="form-checkbox-group"><input id="f-auto" className="form-checkbox" type="checkbox" checked={form.auto_start} onChange={e => setForm(f => ({ ...f, auto_start: e.target.checked }))} /><label className="form-checkbox-label" htmlFor="f-auto">Run at startup</label></div>
          <div className="form-actions"><button className="btn-cancel" onClick={closeForm}>Cancel</button><button className="btn-save" onClick={handleSave}>{editingId ? "Save" : "Add"}</button></div>
        </div>
      </div></div>)}

      {delId && (<div className="confirm-overlay" onClick={() => setDelId(null)}><div className="confirm-box" onClick={e => e.stopPropagation()}>
        <div className="confirm-title">Delete</div><div className="confirm-message">Delete <strong>{commands.find(c => c.id === delId)?.name ?? "this"}</strong>?</div>
        <div className="confirm-actions"><button className="btn-cancel" onClick={() => setDelId(null)}>Cancel</button><button className="btn-danger" onClick={handleDelete}>Delete</button></div>
      </div></div>)}
      {delGroup && (<div className="confirm-overlay" onClick={() => setDelGroup(null)}><div className="confirm-box" onClick={e => e.stopPropagation()}>
        <div className="confirm-title">Delete Group</div><div className="confirm-message">Delete entire group <strong>"{delGroup}"</strong> and all {groups.get(delGroup)?.length ?? 0} commands?</div>
        <div className="confirm-actions"><button className="btn-cancel" onClick={() => setDelGroup(null)}>Cancel</button><button className="btn-danger" onClick={handleDeleteGroup}>Delete All</button></div>
      </div></div>)}

      {toast && <div className={`toast${toast.error ? " toast-error" : ""}`}>{toast.message}</div>}
      {updateInfo && updateInfo.update_available && (
        <div className="toast toast-update">
          <span>Update available: v{updateInfo.latest_version}</span>
          <a className="toast-update-link" href={updateInfo.download_url} target="_blank" rel="noreferrer">Download</a>
          <button className="toast-update-close" onClick={() => setUpdateInfo(null)}>{ICONS.close}</button>
        </div>
      )}
      {showLogs && (
        <div className="modal-overlay" onClick={() => setShowLogs(false)}>
          <div className="modal" style={{ width: 640 }} onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <span className="modal-title">Execution Logs</span>
              <button className="modal-close" onClick={() => setShowLogs(false)}>{ICONS.close}</button>
            </div>
            <div className="modal-body">
              <pre className="log-viewer">{logContent || "No log entries yet."}</pre>
              <div className="form-actions">
                <button className="btn-cancel" onClick={async () => {
                  try { await navigator.clipboard.writeText(logContent); toastMsg("Logs copied"); }
                  catch { toastMsg("Copy failed", true); }
                }}>Copy</button>
                <button className="btn-save" onClick={() => setShowLogs(false)}>Close</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
