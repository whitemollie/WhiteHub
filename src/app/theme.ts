import type { Settings } from "./api";
import { DEFAULTS } from "./format";

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) || 0) as [number, number, number];
}

function mix(a: string, b: string, t: number): string {
  const [x, y] = [hexToRgb(a), hexToRgb(b)];
  return `rgb(${x.map((v, i) => Math.round(v + (y[i] - v) * t)).join(",")})`;
}

/** Publishes the theme as CSS custom properties on <html>. */
export function applyTheme(s: Settings) {
  const a = s.appearance;
  const base = DEFAULTS[a.mode];
  const c = a.colors;
  const bg = c.background ?? base.background;
  const text = c.text ?? base.text;
  const [r, g, b] = hexToRgb(bg);
  const vars: Record<string, string> = {
    "--bg": `rgba(${r},${g},${b},${a.opacity})`,
    "--bg-solid": bg,
    "--card": c.card ?? base.card,
    "--text": text,
    "--muted": c.textMuted ?? base.textMuted,
    "--border": mix(bg, text, a.mode === "dark" ? 0.14 : 0.1),
    "--border-strong": mix(bg, text, a.mode === "dark" ? 0.24 : 0.18),
    "--hover": mix(bg, text, a.mode === "dark" ? 0.06 : 0.04),
    "--track": mix(bg, text, a.mode === "dark" ? 0.12 : 0.1),
    "--accent": c.accent ?? base.accent,
    "--high": c.high ?? DEFAULTS.high,
    "--mid": c.mid ?? DEFAULTS.mid,
    "--low": c.low ?? DEFAULTS.low,
  };
  const root = document.documentElement;
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
  root.dataset.theme = a.mode;
}
