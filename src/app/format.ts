import type { Device, DeviceKind, Level, Settings } from "./api";

export const KINDS: { value: DeviceKind; label: string }[] = [
  { value: "mouse", label: "Mouse" },
  { value: "keyboard", label: "Keyboard" },
  { value: "headset", label: "Headset" },
  { value: "gamepad", label: "Gamepad" },
  { value: "other", label: "Other" },
];

const LEVELS: Record<Level, { label: string; percent: number }> = {
  empty: { label: "Empty", percent: 0 },
  low: { label: "Low", percent: 20 },
  medium: { label: "Medium", percent: 55 },
  full: { label: "Full", percent: 100 },
};

/** Battery value as text; null if unknown. */
export function valueText(d: Device): string | null {
  return d.percent !== null ? `${d.percent}%` : d.level ? LEVELS[d.level].label : null;
}

/** Percentage for the bar; steps are placed where their colour fits. */
export function barPercent(d: Device): number | null {
  return d.percent ?? (d.level ? LEVELS[d.level].percent : null);
}

export const DEFAULTS = {
  dark: { accent: "#ededed", background: "#0a0a0a", card: "#111111", text: "#ededed", textMuted: "#a1a1a1" },
  light: { accent: "#0a0a0a", background: "#ffffff", card: "#fafafa", text: "#0a0a0a", textMuted: "#666666" },
  high: "#22c55e",
  mid: "#eab308",
  low: "#ef4444",
};

export function relativeTime(ms: number | null, now: number): string {
  if (!ms) return "Never";
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 45) return "Just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  return new Date(ms).toLocaleDateString();
}

export function clock(ms: number | null): string {
  return ms ? new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "—";
}

export function statusText(d: Device): string {
  switch (d.status) {
    case "unknown":
      return "Checking";
    case "online":
      return d.charging ? "Charging" : "Connected";
    case "asleep":
      return d.discovered ? "No level" : "Asleep";
    case "off":
      return "Device off";
    case "disconnected":
      return "Disconnected";
    case "error":
      return "Error";
    case "disabled":
      return "Disabled";
  }
}

/** Where the shown value comes from. Not a live stream: it is as old as the last reading. */
export function sourceText(d: Device): string {
  if (d.status === "disabled") return "Monitoring off";
  if (d.status === "online") return d.discovered ? "Reported by Windows" : "Read from device";
  if (valueText(d) !== null) return "Last known value";
  if (d.status === "unknown") return "Waiting for first reading";
  return "No reading yet";
}

export type Tone = "ok" | "warn" | "off" | "err";
export function statusTone(d: Device): Tone {
  switch (d.status) {
    case "online":
      return "ok";
    case "asleep":
    case "unknown":
      return "warn";
    case "error":
      return "err";
    default:
      return "off";
  }
}

export function levelColor(s: Settings, p: number): string {
  const a = s.appearance;
  if (p >= a.highThreshold) return a.colors.high ?? DEFAULTS.high;
  if (p >= a.lowThreshold) return a.colors.mid ?? DEFAULTS.mid;
  return a.colors.low ?? DEFAULTS.low;
}
