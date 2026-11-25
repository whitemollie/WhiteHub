import { createSignal } from "solid-js";
import { createStore, produce, reconcile } from "solid-js/store";
import { api, type AppInfo, type Device, type Settings } from "./api";

export type Page = "dashboard" | "devices" | "appearance" | "settings" | "about";
export const PAGES: Page[] = ["dashboard", "devices", "appearance", "settings", "about"];

interface State {
  ready: boolean;
  devices: Device[];
  settings: Settings | null;
  app: AppInfo | null;
}

const [state, setState] = createStore<State>({ ready: false, devices: [], settings: null, app: null });
export { state };

export const [page, setPageSignal] = createSignal<Page>("dashboard");
export const [now, setNow] = createSignal(Date.now());
export const [saveError, setSaveError] = createSignal<string | null>(null);

export async function load() {
  const s = await api.getState();
  const start = (s.initialPage ?? s.settings.ui.lastPage) as Page;
  if (PAGES.includes(start)) setPageSignal(start);
  setState({ ready: true, devices: s.devices, settings: s.settings, app: s.app });
  api.onDevices((d) => {
    setNow(Date.now());
    setState("devices", reconcile(d, { key: "id" }));
  });
  api.onNavigate((p) => PAGES.includes(p as Page) && navigate(p as Page));
  setInterval(() => setNow(Date.now()), 30_000);
}

let timer: number | undefined;

/** Applies a change locally right away and persists it (debounced, so sliders stay smooth). */
export function updateSettings(fn: (s: Settings) => void, delay = 250) {
  setState(produce((st) => st.settings && fn(st.settings)));
  clearTimeout(timer);
  timer = window.setTimeout(async () => {
    if (!state.settings) return;
    try {
      const saved = await api.updateSettings(JSON.parse(JSON.stringify(state.settings)));
      setState("settings", reconcile(saved));
      setSaveError(null);
    } catch (e) {
      setSaveError(String(e));
    }
  }, delay);
}

export function navigate(p: Page) {
  // Page transitions only between tabs: the first page must be complete when the window appears.
  document.documentElement.dataset.live = "";
  setPageSignal(p);
  if (state.settings && state.settings.ui.lastPage !== p) updateSettings((s) => (s.ui.lastPage = p), 600);
}

export function deviceSettings(id: string) {
  return state.settings?.devices[id];
}
