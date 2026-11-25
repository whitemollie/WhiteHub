import { createSignal, For, Show } from "solid-js";
import { api, type Device } from "../app/api";
import { clock, KINDS, relativeTime, sourceText, statusText, statusTone, valueText } from "../app/format";
import { deviceSettings, forgetDevice, now, state, updateSettings } from "../app/store";
import { Button, Row, Section, Segmented, Slider, Toggle } from "../components/Controls";
import { DeviceGlyph } from "../components/DeviceCard";
import { RefreshIcon } from "../components/Icons";

export function Devices() {
  return (
    <div class="page">
      <header class="page-head">
        <div>
          <h1>Devices</h1>
          <p class="muted">Supported receivers, Bluetooth devices and controllers appear by themselves.</p>
        </div>
      </header>
      <For each={state.devices.filter((d) => d.known && deviceSettings(d.id))}>{(d) => <DeviceSection device={d} />}</For>
      <MissingDevice />
    </div>
  );
}

function MissingDevice() {
  const [result, setResult] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const run = async () => {
    setBusy(true);
    try {
      setResult(`Saved to ${await api.exportDeviceRequest()}`);
    } catch (e) {
      setResult(String(e));
    }
    setBusy(false);
  };
  return (
    <Section title="My device is missing">
      <Row stack title="Create a device request" hint="Lists connected hardware without serial numbers, addresses or Bluetooth names. Nothing is sent: check the file, then attach it to a request on GitHub.">
        <Button onClick={run} disabled={busy()}>
          {busy() ? "Collecting…" : "Create file"}
        </Button>
      </Row>
      <Show when={result()}>
        <p class="hint-box">{result()}</p>
      </Show>
    </Section>
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
            {valueText(d()) ?? "—"}
            <Show when={d().millivolts}> · {(d().millivolts! / 1000).toFixed(2)} V</Show>
            <span class="muted">
              {" "}
              · {sourceText(d())}
              <Show when={d().lastUpdate}>, {relativeTime(d().lastUpdate, now()).toLowerCase()}</Show>
            </span>
          </dd>
          <Show when={!d().discovered}>
            <dt>Receiver</dt>
            <dd>
              {d().product ?? "Not found"} <span class="mono muted">{d().hardware ?? ""}</span>
            </dd>
          </Show>
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
      <Show when={d().status === "asleep" && d().kind === "mouse" && !d().discovered}>
        <p class="hint-box">The receiver only reports the level while the mouse is awake. Move it and refresh.</p>
      </Show>
      <Show when={d().status === "error"}>
        <p class="error-text">{d().error}</p>
      </Show>
      <div class="device-actions btn-row">
        <Button onClick={refresh} disabled={!cfg().enabled || busy()}>
          <RefreshIcon size={14} /> {busy() ? "Refreshing…" : "Refresh now"}
        </Button>
        <Show when={d().discovered && d().status !== "online"}>
          <Button variant="ghost" onClick={() => forgetDevice(d().id)} title="Remove from WhiteHub. The device stays paired in Windows.">
            Forget
          </Button>
        </Show>
      </div>
      <div class="divider" />
      <Row title="Show on taskbar" hint="Include this device in the battery indicator.">
        <Toggle label="Show on taskbar" checked={cfg().showInIndicator} onChange={(v) => set((c) => (c.showInIndicator = v), 0)} />
      </Row>
      <Row title="Low battery alert" hint="One notification when the level drops to the threshold.">
        <Toggle label="Low battery alert" checked={cfg().notifyLow} onChange={(v) => set((c) => (c.notifyLow = v), 0)} />
      </Row>
      <Show when={cfg().notifyLow && !d().level}>
        <Row title="Alert threshold">
          <Slider label="Alert threshold" min={5} max={50} step={5} value={cfg().lowThreshold} format={(v) => `${v}%`} onInput={(v) => set((c) => (c.lowThreshold = v))} />
        </Row>
      </Show>
      <Row title="Display name">
        <input
          class="input"
          placeholder={d().defaultName}
          value={cfg().name ?? ""}
          maxLength={40}
          onChange={(e) => set((c) => (c.name = e.currentTarget.value.trim() || null), 0)}
        />
      </Row>
      <Show when={d().discovered}>
        <Row stack title="Device type" hint="Sets the icon, if Windows reports the wrong type.">
          <Segmented label="Device type" value={d().kind} options={KINDS} onChange={(v) => set((c) => (c.kind = v), 0)} />
        </Row>
      </Show>
      <Show when={!d().discovered}>
      <Row title="USB product ID" hint="Optional. Use only this receiver, e.g. A860. Other models are untested.">
        <input
          class="input mono narrow"
          placeholder="Auto"
          value={cfg().productId ?? ""}
          maxLength={6}
          onChange={(e) => set((c) => (c.productId = e.currentTarget.value.trim() || null), 0)}
        />
      </Row>
      </Show>
    </Section>
  );
}
