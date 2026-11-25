import { state } from "../app/store";
import { LogoMark } from "../components/Icons";

export function About() {
  return (
    <div class="page">
      <div class="about-hero">
        <div class="about-logo">
          <LogoMark size={28} />
        </div>
        <div>
          <h1>WhiteHub</h1>
          <p class="muted">Version {state.app?.version} · by whitemollie</p>
        </div>
      </div>
      <p class="about-text">
        A small Windows utility that reads battery levels straight from your wireless receivers and shows them on the taskbar. It runs
        locally: no accounts, no telemetry, no network requests, and it never modifies vendor software.
      </p>
      <dl class="info-grid about-grid">
        <dt>Author</dt>
        <dd>whitemollie</dd>
        <dt>Supported</dt>
        <dd>WLmouse Beast G · HyperX Cloud III S Wireless · Bluetooth devices whose battery Windows knows · Xbox controllers</dd>
        <dt>Method</dt>
        <dd>Read-only HID queries for the two receivers; for everything else the level Windows itself reports.</dd>
        <dt>Settings</dt>
        <dd class="mono small">{state.app?.configPath}</dd>
        <dt>Logs</dt>
        <dd class="mono small">{state.app?.logDir}</dd>
        <dt>Licence</dt>
        <dd>GPL-3.0, no warranty. Source and licence text are in the project repository.</dd>
        <dt>Credits</dt>
        <dd class="small">Protocols: BatteryHub by ferloom (GPL-3.0), HyperHeadset, hyperx-battery-logger and HyperX-Battery-Tray (MIT).</dd>
      </dl>
    </div>
  );
}
