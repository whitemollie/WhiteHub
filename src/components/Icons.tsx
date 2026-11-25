import type { JSX } from "solid-js";

type P = { size?: number; class?: string };

function Svg(props: P & { children: JSX.Element }) {
  return (
    <svg
      class={props.class}
      width={props.size ?? 16}
      height={props.size ?? 16}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {props.children}
    </svg>
  );
}

export const MouseIcon = (p: P) => (
  <Svg {...p}>
    <rect x="6" y="3" width="12" height="18" rx="6" />
    <path d="M12 7v4" />
  </Svg>
);

export const KeyboardIcon = (p: P) => (
  <Svg {...p}>
    <rect x="2" y="6" width="20" height="12" rx="2.5" />
    <path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" />
  </Svg>
);

export const GamepadIcon = (p: P) => (
  <Svg {...p}>
    <path d="M7.5 7h9a5.5 5.5 0 0 1 5.4 6.6l-.5 2.6a2.6 2.6 0 0 1-4.6 1L15 15H9l-1.8 2.2a2.6 2.6 0 0 1-4.6-1l-.5-2.6A5.5 5.5 0 0 1 7.5 7z" />
    <path d="M8 9.5v3M6.5 11h3M15.5 10h.01M17.5 12h.01" />
  </Svg>
);

export const BluetoothIcon = (p: P) => (
  <Svg {...p}>
    <path d="m7 7 10 10-5 5V2l5 5L7 17" />
  </Svg>
);

export const HeadphonesIcon = (p: P) => (
  <Svg {...p}>
    <path d="M3 18v-6a9 9 0 0 1 18 0v6" />
    <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z" />
  </Svg>
);

export const HeadsetIcon = (p: P) => (
  <Svg {...p}>
    <path d="M3 14v-2a9 9 0 0 1 18 0v2" />
    <path d="M21 15a2 2 0 0 1-2 2h-1v-6h1a2 2 0 0 1 2 2zM3 15a2 2 0 0 0 2 2h1v-6H5a2 2 0 0 0-2 2z" />
    <path d="M19 17v1a4 4 0 0 1-4 4h-3" />
  </Svg>
);

export const GridIcon = (p: P) => (
  <Svg {...p}>
    <rect x="3" y="3" width="7" height="7" rx="1.5" />
    <rect x="14" y="3" width="7" height="7" rx="1.5" />
    <rect x="3" y="14" width="7" height="7" rx="1.5" />
    <rect x="14" y="14" width="7" height="7" rx="1.5" />
  </Svg>
);

export const DevicesIcon = (p: P) => (
  <Svg {...p}>
    <rect x="3" y="4" width="13" height="10" rx="1.5" />
    <path d="M7 18h5" />
    <rect x="17" y="9" width="4" height="11" rx="1" />
  </Svg>
);

export const PaletteIcon = (p: P) => (
  <Svg {...p}>
    <path d="M12 21a9 9 0 1 1 9-9c0 2.5-2 3.5-3.5 3.5H15a2 2 0 0 0-1.5 3.3c.4.5.5 1 .5 1.2 0 .6-.9 1-2 1z" />
    <circle cx="7.5" cy="11" r="1" />
    <circle cx="10.5" cy="7" r="1" />
    <circle cx="15.5" cy="7.5" r="1" />
  </Svg>
);

export const SlidersIcon = (p: P) => (
  <Svg {...p}>
    <path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0" />
    <circle cx="16" cy="6" r="2" />
    <circle cx="10" cy="12" r="2" />
    <circle cx="18" cy="18" r="2" />
  </Svg>
);

export const InfoIcon = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 8h.01" />
  </Svg>
);

export const RefreshIcon = (p: P) => (
  <Svg {...p}>
    <path d="M20 11a8 8 0 0 0-14.9-3.5M4 4v4h4M4 13a8 8 0 0 0 14.9 3.5M20 20v-4h-4" />
  </Svg>
);

export const CloseIcon = (p: P) => (
  <Svg {...p}>
    <path d="M6 6l12 12M18 6 6 18" />
  </Svg>
);

/** Filled, sharp charging bolt — same outline as the taskbar strip. */
export const BoltIcon = (p: P) => (
  <svg class={p.class} width={p.size ?? 16} height={p.size ?? 16} viewBox="0 0 24 24" aria-hidden="true">
    <path d="M14.73 2 6.05 13.6h5.33L9.27 22l8.68-11.6h-5.33z" fill="currentColor" />
  </svg>
);

export const LogoMark = (p: P) => (
  <svg width={p.size ?? 18} height={p.size ?? 18} viewBox="0 0 1024 1024" aria-hidden="true">
    <rect x="196" y="352" width="568" height="320" rx="72" fill="none" stroke="currentColor" stroke-width="64" />
    <rect x="788" y="452" width="56" height="120" rx="20" fill="currentColor" />
    <rect x="268" y="424" width="248" height="176" rx="28" fill="currentColor" />
  </svg>
);
