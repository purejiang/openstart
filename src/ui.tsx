import { useEffect } from "react";
import type { ReactNode } from "react";
import { IconAlert, IconX } from "./icons";

/* ── Switch ──────────────────────────────────────────────────────────── */

export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: () => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={label}
      disabled={disabled}
      className="switch"
      onClick={onChange}
    />
  );
}

/* ── Icon button ─────────────────────────────────────────────────────── */

type IconVariant = "ghost" | "accent" | "danger";

export function IconButton({
  label,
  onClick,
  disabled,
  variant = "ghost",
  className,
  children,
}: {
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  variant?: IconVariant;
  className?: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`icon-btn icon-btn-${variant}${className ? ` ${className}` : ""}`}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/* ── Modal ───────────────────────────────────────────────────────────── */

export function Modal({
  title,
  subtitle,
  onClose,
  children,
  footer,
  wide,
  dismissOnBackdrop = true,
}: {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  dismissOnBackdrop?: boolean;
}) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="overlay"
      onMouseDown={(event) => {
        if (dismissOnBackdrop && event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className={`modal${wide ? " modal-wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="modal-head">
          <div className="modal-head-text">
            <h2 className="modal-title">{title}</h2>
            {subtitle && <p className="modal-subtitle">{subtitle}</p>}
          </div>
          <IconButton label="Close dialog" onClick={onClose}>
            <IconX />
          </IconButton>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  );
}

/* ── Confirm dialog ──────────────────────────────────────────────────── */

export function ConfirmDialog({
  title,
  message,
  confirmLabel = "Delete",
  onCancel,
  onConfirm,
}: {
  title: string;
  message: ReactNode;
  confirmLabel?: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="overlay" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onCancel();
    }}>
      <div className="confirm" role="alertdialog" aria-modal="true" aria-label={title}>
        <div className="confirm-icon">
          <IconAlert size={18} />
        </div>
        <h2 className="confirm-title">{title}</h2>
        <div className="confirm-message">{message}</div>
        <div className="confirm-actions">
          <button className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
          <button className="btn btn-danger" onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
