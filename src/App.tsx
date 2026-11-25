import { createEffect, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
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

export function App() {
  let content!: HTMLElement;

  createEffect(() => state.settings && applyTheme(state.settings));
  // The window keeps one size, so every page starts at its top.
  createEffect(on(page, () => content.scrollTo(0, 0), { defer: true }));

  const close = () => api.closeWindow();

  onMount(() => {
    // Two frames: the first lays the page out, the second follows its paint. The timer covers a
    // WebView that pauses animation frames while its window is still cloaked.
    let sent = false;
    const ready = () => {
      if (sent) return;
      sent = true;
      api.windowReady();
    };
    const timer = setTimeout(ready, 250);
    requestAnimationFrame(() => requestAnimationFrame(ready));

    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    onCleanup(() => {
      clearTimeout(timer);
      window.removeEventListener("keydown", onKey);
    });
  });

  return (
    <div class="shell">
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
        <main class="content" ref={content}>
          <div class="content-inner">
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
          <Sidebar />
        </aside>
      </div>
    </div>
  );
}
