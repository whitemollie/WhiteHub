import { createSignal, For, Show } from "solid-js";
import { api } from "../app/api";
import { navigate, state } from "../app/store";
import { DeviceCard } from "../components/DeviceCard";
import { RefreshIcon } from "../components/Icons";

export function Dashboard() {
  const [busy, setBusy] = createSignal(false);
  const devices = () => state.devices.filter((d) => d.known && d.enabled);
  const refresh = async () => {
    setBusy(true);
    await api.refresh();
    setTimeout(() => setBusy(false), 1500);
  };
  return (
    <div class="page">
      <header class="page-head">
        <div>
          <h1>Dashboard</h1>
          <p class="muted">Every {state.settings?.pollIntervalMinutes} min · devices appear by themselves</p>
        </div>
        <button type="button" class="btn icon-btn" classList={{ spinning: busy() }} onClick={refresh} title="Refresh now" aria-label="Refresh now">
          <RefreshIcon />
        </button>
      </header>
      <Show when={state.app?.stripAvailable === false && state.settings?.indicators.mode === "taskbar"}>
        <div class="notice">The taskbar strip could not be placed, so battery levels are shown as tray icons instead.</div>
      </Show>
      <div class="cards">
        <For each={devices()} fallback={<EmptyState />}>
          {(d) => <DeviceCard device={d} />}
        </For>
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div class="empty">
      <Show
        when={!state.devices.some((d) => d.known)}
        fallback={
          <>
            <p>All devices are disabled.</p>
            <button type="button" class="link" onClick={() => navigate("devices")}>
              Manage devices
            </button>
          </>
        }
      >
        <p>No device found yet. Connect one, it appears by itself:</p>
        <ul class="empty-list">
          <li>Bluetooth: any device whose battery Windows shows</li>
          <li>2.4 GHz dongle: supported models, see About</li>
          <li>Xbox wireless controllers</li>
        </ul>
        <button type="button" class="link" onClick={() => navigate("devices")}>
          My device is missing
        </button>
      </Show>
    </div>
  );
}
