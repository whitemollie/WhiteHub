import { createEffect, createSignal, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import { api } from "./app/api";
import { page, saveError, state } from "./app/store";
import { applyTheme } from "./app/theme";
import { CloseIcon, LogoMark } from "./components/Icons";
import { Sidebar } from "./components/Sidebar";
import { About } from "./pages/About";
import { Appearance } from "./pages/Appearance";
import { Dashboard } from "./pages/Dashboard";
import { Devices } from "./pages/Devices";
import { Settings } from "./pages/Settings";

const TITLEBAR = 44;

export function App() {
  const [closing, setClosing] = createSignal(false);
  let inner!: HTMLDivElement;
  let side!: HTMLDivElement;

  createEffect(() => state.settings && applyTheme(state.settings));

  const close = () => {
    if (closing()) return;
    setClosing(true);
    setTimeout(() => api.closeWindow(), 140);
  };

  onMount(() => {
    // Size the native window to the content (the backend clamps and keeps it above the taskbar).
    // ResizeObserver callbacks run after layout, so no extra frame is needed.
    let last = 0;
    const fit = () => {
      const h = Math.ceil(TITLEBAR + Math.max(inner?.scrollHeight ?? 0, side?.scrollHeight ?? 0) + 2);
      if (Math.abs(h - last) > 2) {
        last = h;
        api.fitWindow(h);
      }
    };
    const ro = new ResizeObserver(fit);
    ro.observe(inner);
    ro.observe(side);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    onCleanup(() => {
      ro.disconnect();
      window.removeEventListener("keydown", onKey);
    });
  });

  return (
    <div class="shell" classList={{ closing: closing() }}>
      <header class="titlebar" data-tauri-drag-region>
        <div class="brand" data-tauri-drag-region>
          <LogoMark size={16} />
          <span data-tauri-drag-region>WhiteHub</span>
        </div>
        <button type="button" class="icon-btn close" onClick={close} aria-label="Close" title="Close (Esc)">
          <CloseIcon size={15} />
        </button>
      </header>
      <div class="body">
        <main class="content">
          <div class="content-inner" ref={inner}>
            <Show when={saveError()}>
              <div class="notice err">{saveError()}</div>
            </Show>
            <Show when={state.ready && state.settings}>
              <Switch>
                <Match when={page() === "dashboard"}>
                  <Dashboard />
                </Match>
                <Match when={page() === "devices"}>
                  <Devices />
                </Match>
                <Match when={page() === "appearance"}>
                  <Appearance />
                </Match>
                <Match when={page() === "settings"}>
                  <Settings />
                </Match>
                <Match when={page() === "about"}>
                  <About />
                </Match>
              </Switch>
            </Show>
          </div>
        </main>
        <aside>
          {/* Measured for the window height; the <aside> itself stretches to the window. */}
          <div ref={side}>
            <Sidebar />
          </div>
        </aside>
      </div>
    </div>
  );
}
