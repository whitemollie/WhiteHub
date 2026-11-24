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
          <p class="muted">Version {state.app?.version}</p>
        </div>
      </div>
      <p class="about-text">
        A small Windows utility that reads battery levels straight from your wireless receivers and shows them on the taskbar. It runs
        locally: no accounts, no telemetry, no network requests, and it never modifies vendor software.
      </p>
      <dl class="info-grid about-grid">
        <dt>Supported</dt>
        <dd>WLmouse Beast G (2.4 GHz receiver 36A7:A860) · HyperX Cloud III S Wireless (dongle 03F0:06BE)</dd>
        <dt>Method</dt>
        <dd>Standard Windows HID driver, vendor-defined reports. The mouse query matches the official WL MOUSE HUB.</dd>
        <dt>Settings</dt>
        <dd class="mono small">{state.app?.configPath}</dd>
        <dt>Logs</dt>
        <dd class="mono small">{state.app?.logDir}</dd>
        <dt>Credits</dt>
        <dd class="small">HyperX protocol documented by HyperX-Battery-Tray, hyperx-battery-logger and HyperHeadset (MIT).</dd>
      </dl>
    </div>
  );
}
