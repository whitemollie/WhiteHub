import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DeviceKind = "mouse" | "headset";
export type Status = "unknown" | "online" | "asleep" | "off" | "disconnected" | "error" | "disabled";

export interface Device {
  id: string;
  name: string;
  kind: DeviceKind;
  connection: string;
  status: Status;
  percent: number | null;
  charging: boolean;
  millivolts: number | null;
  lastUpdate: number | null;
  lastAttempt: number | null;
  source: string;
  hardware: string | null;
  product: string | null;
  error: string | null;
  enabled: boolean;
}

export type ThemeMode = "dark" | "light";
export type IndicatorMode = "taskbar" | "tray";
export type Colorize = "off" | "lowOnly" | "all";
export type IconSet = "fluent" | "alternate" | "text";

export interface Colors {
  accent: string | null;
  background: string | null;
  card: string | null;
  text: string | null;
  textMuted: string | null;
  high: string | null;
  mid: string | null;
  low: string | null;
}

export interface DeviceSettings {
  enabled: boolean;
  name: string | null;
  productId: string | null;
  notifyLow: boolean;
  lowThreshold: number;
  showInIndicator: boolean;
  customIcon: string | null;
}

export interface Settings {
  version: number;
  pollIntervalMinutes: number;
  autostart: boolean;
  debugLogging: boolean;
  appearance: {
    mode: ThemeMode;
    colors: Colors;
    opacity: number;
    iconSet: IconSet;
    highThreshold: number;
    lowThreshold: number;
  };
  indicators: { mode: IndicatorMode; colorize: Colorize };
  notifications: { doNotDisturb: boolean; onConnect: boolean; onDisconnect: boolean; onLow: boolean };
  devices: Record<string, DeviceSettings>;
  ui: { lastPage: string };
}

export interface AppInfo {
  version: string;
  configPath: string;
  logDir: string;
  stripAvailable: boolean | null;
}

export interface FullState {
  devices: Device[];
  settings: Settings;
  app: AppInfo;
  initialPage: string | null;
}

export const api = {
  getState: () => invoke<FullState>("get_state"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  defaultSettings: () => invoke<Settings>("default_settings"),
  refresh: (id?: string) => invoke<void>("refresh", { id: id ?? null }),
  fitWindow: (height: number) => invoke<void>("fit_window", { height }),
  closeWindow: () => invoke<void>("close_window"),
  exportDiagnostics: () => invoke<string>("export_diagnostics"),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  quit: () => invoke<void>("quit"),
  onDevices: (cb: (d: Device[]) => void): Promise<UnlistenFn> => listen<Device[]>("devices", (e) => cb(e.payload)),
  onNavigate: (cb: (page: string) => void): Promise<UnlistenFn> => listen<string>("navigate", (e) => cb(e.payload)),
};
