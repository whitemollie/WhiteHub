import { createSignal, Show } from "solid-js";
import { api } from "../app/api";
import { state, updateSettings } from "../app/store";
import { Button, Row, Section, Segmented, Toggle } from "../components/Controls";

export function Settings() {
  const s = () => state.settings!;
  const n = () => s().notifications;
  const [exported, setExported] = createSignal<string | null>(null);
  return (
    <div class="page">
      <header class="page-head">
        <div>
          <h1>Settings</h1>
          <p class="muted">Polling, notifications and startup.</p>
        </div>
      </header>

      <Section title="Monitoring">
        <Row stack title="Check battery every" hint="Fewer checks mean less radio traffic. Opening this window refreshes stale data.">
          <Segmented<number>
            label="Polling interval"
            value={s().pollIntervalMinutes}
            options={[5, 10, 15, 30, 60].map((m) => ({ value: m, label: `${m} min` }))}
            onChange={(v) => updateSettings((x) => (x.pollIntervalMinutes = v), 0)}
          />
        </Row>
        <Row title="Start with Windows" hint="Runs quietly in the background after sign-in.">
          <Toggle label="Start with Windows" checked={s().autostart} onChange={(v) => updateSettings((x) => (x.autostart = v), 0)} />
        </Row>
      </Section>

      <Section title="Notifications">
        <Row title="Do not disturb" hint="Silence all WhiteHub notifications.">
          <Toggle label="Do not disturb" checked={n().doNotDisturb} onChange={(v) => updateSettings((x) => (x.notifications.doNotDisturb = v), 0)} />
        </Row>
        <Row title="Low battery" hint="Threshold per device on the Devices page.">
          <Toggle label="Low battery notifications" disabled={n().doNotDisturb} checked={n().onLow} onChange={(v) => updateSettings((x) => (x.notifications.onLow = v), 0)} />
        </Row>
        <Row title="Device connected">
          <Toggle label="Connected notifications" disabled={n().doNotDisturb} checked={n().onConnect} onChange={(v) => updateSettings((x) => (x.notifications.onConnect = v), 0)} />
        </Row>
        <Row title="Device disconnected">
          <Toggle label="Disconnected notifications" disabled={n().doNotDisturb} checked={n().onDisconnect} onChange={(v) => updateSettings((x) => (x.notifications.onDisconnect = v), 0)} />
        </Row>
      </Section>

      <Section title="Diagnostics">
        <Row title="Debug logging" hint="Adds raw HID replies to the log. Keep off normally.">
          <Toggle label="Debug logging" checked={s().debugLogging} onChange={(v) => updateSettings((x) => (x.debugLogging = v), 0)} />
        </Row>
        <Row title="Logs">
          <div class="btn-row">
            <Button onClick={() => api.openLogsFolder()}>Open folder</Button>
            <Button onClick={async () => setExported(await api.exportDiagnostics())}>Export report</Button>
          </div>
        </Row>
        <Show when={exported()}>
          <p class="hint-box">Saved to {exported()}</p>
        </Show>
      </Section>

      <Section>
        <Row title="Exit WhiteHub" hint="Stops monitoring and removes the indicators until next start.">
          <Button variant="danger" onClick={() => api.quit()}>
            Exit
          </Button>
        </Row>
      </Section>
    </div>
  );
}
