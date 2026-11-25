import type { ReactNode } from "react";

/** Shared 24px stroke-icon frame (Lucide geometry) — one visual language. */
function Svg({ size = 16, children }: { size?: number; children: ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      style={{ flexShrink: 0, display: "block" }}
    >
      {children}
    </svg>
  );
}

export function BrandMark({ size = 24 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      focusable="false"
      style={{ display: "block" }}
    >
      <rect x="0.75" y="0.75" width="22.5" height="22.5" rx="6" fill="#16233a" stroke="rgba(34,197,94,.35)" strokeWidth="1.5" />
      <path d="M13.4 4.2 6.2 13.1h4.6l-.9 6.7 7.2-8.9h-4.6l.9-6.7Z" fill="#22c55e" />
    </svg>
  );
}

export const IconPlus = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);

export const IconPlay = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M7 4.5v15l13-7.5-13-7.5Z" fill="currentColor" stroke="none" />
  </Svg>
);

export const IconStop = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <rect x="5.5" y="5.5" width="13" height="13" rx="2" fill="currentColor" stroke="none" />
  </Svg>
);

export const IconEdit = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M12 20h9" />
    <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
  </Svg>
);

export const IconTrash = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M3 6h18" />
    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
    <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
  </Svg>
);

export const IconX = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M18 6 6 18M6 6l12 12" />
  </Svg>
);

export const IconChevronDown = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="m6 9 6 6 6-6" />
  </Svg>
);

export const IconSearch = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <circle cx="11" cy="11" r="7.5" />
    <path d="m21 21-4.3-4.3" />
  </Svg>
);

export const IconTerminal = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="m4 17 6-6-6-6" />
    <path d="M12 19h8" />
  </Svg>
);

export const IconSettings = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.08a1.7 1.7 0 0 0-1.11-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.08A1.7 1.7 0 0 0 4.6 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 8.96 4.6 1.7 1.7 0 0 0 10 3.04V3a2 2 0 1 1 4 0v.08a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V8.9c.24.6.82 1 1.47 1.03H21a2 2 0 1 1 0 4h-.08a1.7 1.7 0 0 0-1.52 1.07Z" />
  </Svg>
);

export const IconGrip = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <circle cx="9" cy="5" r="1" fill="currentColor" stroke="none" />
    <circle cx="9" cy="12" r="1" fill="currentColor" stroke="none" />
    <circle cx="9" cy="19" r="1" fill="currentColor" stroke="none" />
    <circle cx="15" cy="5" r="1" fill="currentColor" stroke="none" />
    <circle cx="15" cy="12" r="1" fill="currentColor" stroke="none" />
    <circle cx="15" cy="19" r="1" fill="currentColor" stroke="none" />
  </Svg>
);

export const IconCheck = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M20 6 9 17l-5-5" />
  </Svg>
);

export const IconAlert = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
    <path d="M12 9v4M12 17h.01" />
  </Svg>
);

export const IconRefresh = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M21 12a9 9 0 1 1-2.64-6.36" />
    <path d="M21 3v6h-6" />
  </Svg>
);

export const IconLogs = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z" />
    <path d="M14 2v6h6M8 13h8M8 17h5" />
  </Svg>
);

export const IconExternal = ({ size }: { size?: number }) => (
  <Svg size={size}>
    <path d="M15 3h6v6" />
    <path d="M10 14 21 3" />
    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
  </Svg>
);
