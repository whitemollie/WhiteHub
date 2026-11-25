import { createSignal, For, Show } from "solid-js";
import { api, type Device } from "../app/api";
import { clock, relativeTime, sourceText, statusText, statusTone } from "../app/format";
import { deviceSettings, now, state, updateSettings } from "../app/store";
import { Button, Row, Section, Slider, Toggle } from "../components/Controls";
import { DeviceGlyph } from "../components/DeviceCard";
import { RefreshIcon } from "../components/Icons";

export function Devices() {
  return (
    <div class="page">
      <header class="page-head">
        <div>
          <h1>Devices</h1>
          <p class="muted">Detected automatically over USB HID. No vendor software needed.</p>
        </div>
      </header>
      <For each={state.devices}>{(d) => <DeviceSection device={d} />}</For>
    </div>
  );
}

function DeviceSection(props: { device: Device }) {
  const d = () => props.device;
  const cfg = () => deviceSettings(d().id)!;
  const set = (fn: (c: NonNullable<ReturnType<typeof deviceSettings>>) => void, delay?: number) =>
    updateSettings((s) => fn(s.devices[d().id]), delay);
  const [busy, setBusy] = createSignal(false);
  const refresh = async () => {
    setBusy(true);
    await api.refresh(d().id);
    setTimeout(() => setBusy(false), 1500);
  };
  return (
    <Section
      title={d().name}
      action={<Toggle label={`Monitor ${d().name}`} checked={cfg().enabled} onChange={(v) => set((c) => (c.enabled = v), 0)} />}
    >
      <div class="device-info">
        <div class="device-icon lg">
          <DeviceGlyph device={d()} size={22} />
        </div>
        <dl class="info-grid">
          <dt>Status</dt>
          <dd>
            <span class={`pill ${statusTone(d())}`}>
              <span class="dot" />
              {statusText(d())}
            </span>
          </dd>
          <dt>Battery</dt>
          <dd>
            {d().percent !== null ? `${d().percent}%` : "—"}
            <Show when={d().millivolts}> · {(d().millivolts! / 1000).toFixed(2)} V</Show>
            <span class="muted">
              {" "}
              · {sourceText(d())}
              <Show when={d().lastUpdate}>, {relativeTime(d().lastUpdate, now()).toLowerCase()}</Show>
            </span>
          </dd>
          <dt>Receiver</dt>
          <dd>
            {d().product ?? "Not found"} <span class="mono muted">{d().hardware ?? ""}</span>
          </dd>
          <dt>Connection</dt>
          <dd>{d().connection}</dd>
          <dt>Data source</dt>
          <dd class="small">{d().source || "—"}</dd>
          <dt>Last reading</dt>
          <dd>
            {clock(d().lastUpdate)}
            {/* Only worth showing when the latest attempt did not produce a reading. */}
            <Show when={d().lastAttempt !== d().lastUpdate}>
              <span class="muted"> · last attempt {clock(d().lastAttempt)}</span>
            </Show>
          </dd>
        </dl>
      </div>
      <Show when={d().status === "asleep"}>
        <p class="hint-box">The receiver only reports the level while the mouse is awake. Move it and refresh.</p>
      </Show>
      <Show when={d().status === "error"}>
        <p class="error-text">{d().error}</p>
      </Show>
      <div class="device-actions">
        <Button onClick={refresh} disabled={!cfg().enabled || busy()}>
          <RefreshIcon size={14} /> {busy() ? "Refreshing…" : "Refresh now"}
        </Button>
      </div>
      <div class="divider" />
      <Row title="Show on taskbar" hint="Include this device in the battery indicator.">
        <Toggle label="Show on taskbar" checked={cfg().showInIndicator} onChange={(v) => set((c) => (c.showInIndicator = v), 0)} />
      </Row>
      <Row title="Low battery alert" hint="One notification when the level drops to the threshold.">
        <Toggle label="Low battery alert" checked={cfg().notifyLow} onChange={(v) => set((c) => (c.notifyLow = v), 0)} />
      </Row>
      <Show when={cfg().notifyLow}>
        <Row title="Alert threshold">
          <Slider label="Alert threshold" min={5} max={50} step={5} value={cfg().lowThreshold} format={(v) => `${v}%`} onInput={(v) => set((c) => (c.lowThreshold = v))} />
        </Row>
      </Show>
      <Row title="Display name">
        <input
          class="input"
          placeholder={d().id === "wlmouse" ? "WLmouse Beast G" : "HyperX Cloud III S Wireless"}
          value={cfg().name ?? ""}
          maxLength={40}
          onChange={(e) => set((c) => (c.name = e.currentTarget.value.trim() || null), 0)}
        />
      </Row>
      <Row title="USB product ID" hint="Optional. Limit discovery to one receiver, e.g. A860.">
        <input
          class="input mono narrow"
          placeholder="Auto"
          value={cfg().productId ?? ""}
          maxLength={6}
          onChange={(e) => set((c) => (c.productId = e.currentTarget.value.trim() || null), 0)}
        />
      </Row>
    </Section>
  );
}
