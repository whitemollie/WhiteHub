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
        Shows the battery levels of wireless devices on the Windows taskbar. It runs locally: no accounts, no telemetry, no network requests.
      </p>
      <dl class="info-grid about-grid">
        <dt>Author</dt>
        <dd>whitemollie</dd>
        <dt>Supported</dt>
        <dd>WLmouse Beast G · HyperX Cloud III S Wireless · Bluetooth devices whose battery Windows knows · Xbox controllers</dd>
        <dt>Method</dt>
        <dd>Read-only queries for the 2.4 GHz models; for everything else the level Windows itself reports.</dd>
        <dt>Licence</dt>
        <dd>GPL-3.0, no warranty. Source and licence text are in the project repository.</dd>
        <dt>Credits</dt>
        <dd>The projects WhiteHub builds on are listed in CREDITS.md in the project repository.</dd>
      </dl>
    </div>
  );
}
