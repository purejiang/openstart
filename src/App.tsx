import { useCallback, useEffect, useState } from "react";
import type { Command, TerminalInfo, AppSettings, AppInfo } from "./types";
import {
  addCommand, deleteCommand, deleteGroup, detectTerminals, executeCommand, executeGroup,
  getAppInfo, getSettings, listCommands, updateCommand, updateSettings,
} from "./api";
import "./App.css";

const ICONS = {
  plus: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>),
  play: (<svg viewBox="0 0 24 24" fill="currentColor" stroke="none"><path d="M8 5.14v14l11-7-11-7z" /></svg>),
  playAll: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polygon points="5 3 19 12 5 21 5 3" /><line x1="10" y1="3" x2="10" y2="21" /></svg>),
  edit: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>),
  trash: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>),
  close: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>),
  zap: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" /></svg>),
  terminal: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" /></svg>),
  gear: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3" /><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" /></svg>),
  chevron: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><polyline points="6 9 12 15 18 9" /></svg>),
} as const;

function terminalClass(id: string): string {
  const m: Record<string, string> = { powershell: "term-powershell", cmd: "term-cmd", gitbash: "term-gitbash", terminal: "term-winterm" };
  return m[id] ?? "term-default";
}
function terminalLabel(term: TerminalInfo): string {
  const m: Record<string, string> = { powershell: "PowerShell", cmd: "CMD", gitbash: "Git Bash", terminal: "WT" };
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

interface CmdForm { name: string; command: string; terminal: string; auto_start: boolean; group_name: string; }
const EMPTY: CmdForm = { name: "", command: "", terminal: "", auto_start: false, group_name: "" };

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
  const [appSettings, setAppSettings] = useState<AppSettings>({ app_autostart: false, startup_delay_seconds: 3 });
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [dirty, setDirty] = useState(false);
  const [toast, setToast] = useState<{ message: string; error?: boolean } | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [cmds, terms, s, info] = await Promise.all([listCommands(), detectTerminals(), getSettings(), getAppInfo()]);
      setCommands(cmds); setTerminals(terms); setAppSettings(s); setAppInfo(info);
    } catch (e) { toastMsg("Failed to load: " + String(e), true); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { refresh(); }, [refresh]);

  function toastMsg(m: string, e?: boolean) { setToast({ message: m, error: e }); setTimeout(() => setToast(null), 3000); }

  function openAdd() {
    const t = terminals.length > 0 ? terminals[0].id : "";
    setForm({ ...EMPTY, terminal: t }); setEditingId(null); setShowForm(true);
  }
  function openEdit(c: Command) {
    setForm({ name: c.name, command: c.command, terminal: c.terminal, auto_start: c.auto_start, group_name: c.group_name });
    setEditingId(c.id); setShowForm(true);
  }
  function closeForm() { setShowForm(false); setEditingId(null); }

  async function handleSave() {
    const { name, command, terminal, auto_start, group_name } = form;
    if (!name.trim()) { toastMsg("Name required", true); return; }
    if (!command.trim()) { toastMsg("Command required", true); return; }
    if (!terminal) { toastMsg("Terminal required", true); return; }
    try {
      if (editingId) {
        await updateCommand(editingId, name.trim(), command.trim(), terminal, auto_start, group_name.trim());
      } else {
        await addCommand(name.trim(), command.trim(), terminal, auto_start, group_name.trim());
      }
      closeForm(); await refresh();
    } catch (e) { toastMsg("Save failed: " + String(e), true); }
  }

  async function handleExec(c: Command) {
    try { await executeCommand(c.name, c.command, c.terminal); toastMsg(`Executed: ${c.name}`); }
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
    try { await updateSettings(appSettings.app_autostart, appSettings.startup_delay_seconds); toastMsg("Saved"); setDirty(false); }
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
                <div className="table-wrapper">
                  <table className="command-table">
                    <thead><tr><th className="col-name">Name</th><th className="col-command">Command</th><th className="col-terminal">Terminal</th><th className="col-auto">Auto</th><th className="col-actions">Actions</th></tr></thead>
                    <tbody>
                      {gcmds.map(c => {
                        const t = terminals.find(x => x.id === c.terminal) ?? ({ id: c.terminal, name: c.terminal } as TerminalInfo);
                        return (
                          <tr key={c.id}>
                            <td className="col-name-cell" title={c.name}>{c.name}</td>
                            <td className="col-command-cell" title={c.command}>{c.command}</td>
                            <td><span className={`terminal-badge ${terminalClass(c.terminal)}`}>{terminalLabel(t)}</span></td>
                            <td><span className="auto-start-indicator"><span className={`auto-start-dot${c.auto_start ? " active" : ""}`} />{c.auto_start ? "Yes" : "No"}</span></td>
                            <td><div className="row-actions">
                              <button className="btn-icon btn-icon-execute" title="Execute" onClick={() => handleExec(c)}>{ICONS.play}</button>
                              <button className="btn-icon btn-icon-edit" title="Edit" onClick={() => openEdit(c)}>{ICONS.edit}</button>
                              <button className="btn-icon btn-icon-delete" title="Delete" onClick={() => setDelId(c.id)}>{ICONS.trash}</button>
                            </div></td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
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
        {appInfo && (<div className="settings-info-grid"><span className="info-label">Name</span><span className="info-value">{appInfo.name}</span><span className="info-label">Version</span><span className="info-value">{appInfo.version}</span><span className="info-label">Data</span><span className="info-value info-value-mono">{appInfo.dataDir}</span></div>)}
      </section>
      <section className="settings-section">
        <h3 className="settings-section-title">Startup</h3>
        <div className="settings-row"><span>Launch at system startup</span><button className={`toggle${appSettings.app_autostart ? " active" : ""}`} onClick={() => { setAppSettings(s => ({ ...s, app_autostart: !s.app_autostart })); setDirty(true); }}><span className="toggle-knob" /></button></div>
        <div className="settings-row"><span>Startup delay (seconds)</span><input type="number" className="settings-input-num" min={0} max={60} value={appSettings.startup_delay_seconds} onChange={e => { setAppSettings(s => ({ ...s, startup_delay_seconds: Number(e.target.value) || 0 })); setDirty(true); }} /></div>
        {dirty && <button className="btn-save" onClick={handleSaveSettings}>Save Settings</button>}
      </section>
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

      {showForm && (<div className="modal-overlay" onClick={closeForm}><div className="modal" onClick={e => e.stopPropagation()}>
        <div className="modal-header"><span className="modal-title">{editingId ? "Edit" : "Add"} Command</span><button className="modal-close" onClick={closeForm}>{ICONS.close}</button></div>
        <div className="modal-body">
          <div className="form-group"><label className="form-label" htmlFor="f-name">Name</label><input id="f-name" className="form-input" placeholder="e.g. Start dev" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} autoFocus /></div>
          <div className="form-group"><label className="form-label" htmlFor="f-cmd">Command</label><input id="f-cmd" className="form-input" placeholder="e.g. npm run dev" value={form.command} onChange={e => setForm(f => ({ ...f, command: e.target.value }))} /></div>
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
    </>
  );
}
