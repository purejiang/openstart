import { useEffect, useState } from "react";
import type { AppInfo, AppSettings } from "./types";
import { errorMessage, getSettings, readLogs, updateSettings } from "./api";
import { Modal, Switch } from "./ui";
import { IconLogs, IconRefresh } from "./icons";

export default function SettingsPage({
  appInfo,
  onToast,
  onCheckUpdate,
}: {
  appInfo: AppInfo | null;
  onToast: (message: string, isError?: boolean) => void;
  onCheckUpdate: () => Promise<void>;
}) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [checking, setChecking] = useState(false);
  const [logs, setLogs] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((value) => {
        if (alive) setSettings(value);
      })
      .catch((error: unknown) => onToast("Failed to load settings: " + errorMessage(error), true));
    return () => {
      alive = false;
    };
  }, [onToast]);

  function patch(next: Partial<AppSettings>) {
    setSettings((current) => (current ? { ...current, ...next } : current));
    setDirty(true);
  }

  async function save() {
    if (!settings) return;
    setSaving(true);
    try {
      await updateSettings(
        settings.app_autostart,
        settings.startup_delay_seconds,
        settings.close_to_tray,
        settings.keep_terminal_open,
      );
      setDirty(false);
      onToast("Settings saved");
    } catch (error) {
      onToast("Save failed: " + errorMessage(error), true);
    } finally {
      setSaving(false);
    }
  }

  async function openLogs() {
    try {
      setLogs(await readLogs(200));
    } catch (error) {
      onToast("Failed to read logs: " + errorMessage(error), true);
    }
  }

  async function checkUpdates() {
    setChecking(true);
    try {
      await onCheckUpdate();
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="page page-settings">
      <header className="page-head">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-sub">Application behavior and startup options</p>
        </div>
      </header>

      <section className="card" aria-labelledby="settings-startup">
        <div className="card-head">
          <h2 className="card-title" id="settings-startup">
            Startup
          </h2>
          <p className="card-desc">When OpenStart launches and how command windows behave.</p>
        </div>
        <div className="set-row">
          <div className="set-row-text">
            <span className="set-row-label">Launch at system startup</span>
            <span className="set-row-desc">Start OpenStart automatically when Windows starts</span>
          </div>
          <Switch
            checked={settings?.app_autostart ?? false}
            label="Launch at system startup"
            onChange={() => patch({ app_autostart: !settings?.app_autostart })}
          />
        </div>
        <div className="set-row">
          <div className="set-row-text">
            <span className="set-row-label">Minimize to tray on close</span>
            <span className="set-row-desc">Closing the window hides it to the tray instead of quitting</span>
          </div>
          <Switch
            checked={settings?.close_to_tray ?? true}
            label="Minimize to tray on close"
            onChange={() => patch({ close_to_tray: !settings?.close_to_tray })}
          />
        </div>
        <div className="set-row">
          <div className="set-row-text">
            <span className="set-row-label">Keep terminal open after execution</span>
            <span className="set-row-desc">A run stays active until its terminal window is closed</span>
          </div>
          <Switch
            checked={settings?.keep_terminal_open ?? true}
            label="Keep terminal open after execution"
            onChange={() => patch({ keep_terminal_open: !settings?.keep_terminal_open })}
          />
        </div>
        <div className="set-row">
          <div className="set-row-text">
            <span className="set-row-label">Startup delay</span>
            <span className="set-row-desc">Wait before auto-running commands at login</span>
          </div>
          <div className="set-num">
            <input
              type="number"
              className="input"
              min={0}
              max={60}
              value={settings?.startup_delay_seconds ?? 0}
              aria-label="Startup delay in seconds"
              onChange={(event) =>
                patch({ startup_delay_seconds: Math.max(0, Math.min(60, Number(event.target.value) || 0)) })
              }
            />
            <span className="set-num-unit">seconds</span>
          </div>
        </div>
      </section>

      <section className="card" aria-labelledby="settings-about">
        <div className="card-head">
          <h2 className="card-title" id="settings-about">
            About
          </h2>
          <p className="card-desc">Build information, data location and diagnostics.</p>
        </div>
        <div className="info-grid">
          <span className="info-label">Name</span>
          <span className="info-value">{appInfo?.name ?? "—"}</span>
          <span className="info-label">Version</span>
          <span className="info-value">
            {appInfo?.version ? `v${appInfo.version}` : "—"}
          </span>
          <span className="info-label">Data folder</span>
          <span className="info-value info-value-mono selectable" title={appInfo?.dataDir}>
            {appInfo?.dataDir ?? "—"}
          </span>
        </div>
        <div className="card-actions">
          <button className="btn btn-primary" onClick={checkUpdates} disabled={checking}>
            <IconRefresh size={14} />
            {checking ? "Checking…" : "Check for updates"}
          </button>
          <button className="btn btn-ghost" onClick={openLogs}>
            <IconLogs size={14} />
            View logs
          </button>
        </div>
      </section>

      {dirty && (
        <div className="savebar">
          <span className="savebar-text">Unsaved changes</span>
          <div className="savebar-actions">
            <button
              className="btn btn-ghost"
              onClick={() => {
                void getSettings()
                  .then((value) => {
                    setSettings(value);
                    setDirty(false);
                  })
                  .catch((error: unknown) => onToast("Failed to reload settings: " + errorMessage(error), true));
              }}
            >
              Reset
            </button>
            <button className="btn btn-primary" onClick={save} disabled={saving}>
              {saving ? "Saving…" : "Save changes"}
            </button>
          </div>
        </div>
      )}

      {logs !== null && (
        <Modal
          title="Execution logs"
          subtitle="Most recent entries first."
          wide
          onClose={() => setLogs(null)}
          footer={
            <>
              <button
                className="btn btn-ghost"
                onClick={async () => {
                  try {
                    await navigator.clipboard.writeText(logs);
                    onToast("Logs copied");
                  } catch {
                    onToast("Copy failed", true);
                  }
                }}
              >
                Copy all
              </button>
              <button className="btn btn-primary" onClick={() => setLogs(null)}>
                Done
              </button>
            </>
          }
        >
          <pre className="log-viewer selectable">{logs || "No log entries yet."}</pre>
        </Modal>
      )}
    </div>
  );
}
