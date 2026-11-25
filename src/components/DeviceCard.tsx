import { Match, Show, Switch } from "solid-js";
import type { Device } from "../app/api";
import { barPercent, clock, levelColor, relativeTime, sourceText, statusText, statusTone, valueText } from "../app/format";
import { deviceSettings, now, state } from "../app/store";
import { BluetoothIcon, BoltIcon, GamepadIcon, HeadphonesIcon, HeadsetIcon, KeyboardIcon, MouseIcon } from "./Icons";

export function DeviceGlyph(props: { device: Device; size?: number }) {
  const custom = () => deviceSettings(props.device.id)?.customIcon;
  const set = () => state.settings?.appearance.iconSet;
  return (
    <Show
      when={custom()}
      fallback={
        <Switch fallback={<BluetoothIcon size={props.size} />}>
          <Match when={props.device.kind === "mouse"}>
            <MouseIcon size={props.size} />
          </Match>
          <Match when={props.device.kind === "keyboard"}>
            <KeyboardIcon size={props.size} />
          </Match>
          <Match when={props.device.kind === "gamepad"}>
            <GamepadIcon size={props.size} />
          </Match>
          <Match when={props.device.kind === "headset"}>
            <Show when={set() === "alternate"} fallback={<HeadphonesIcon size={props.size} />}>
              <HeadsetIcon size={props.size} />
            </Show>
          </Match>
        </Switch>
      }
    >
      <img src={custom()!} alt="" width={props.size} height={props.size} class="custom-icon" />
    </Show>
  );
}

export function BatteryBar(props: { percent: number | null; dim?: boolean }) {
  const color = () => (props.percent === null || !state.settings ? "var(--muted)" : levelColor(state.settings, props.percent));
  return (
    <div class="battery-bar" classList={{ dim: props.dim }} role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={props.percent ?? undefined}>
      <div class="battery-fill" style={{ width: `${props.percent ?? 0}%`, background: color() }} />
    </div>
  );
}

export function DeviceCard(props: { device: Device }) {
  const d = () => props.device;
  const live = () => d().status === "online";
  return (
    <article class="device-card" classList={{ stale: !live() }}>
      <div class="device-top">
        <div class="device-icon">
          <DeviceGlyph device={d()} size={20} />
        </div>
        <div class="device-title">
          <div class="device-name">{d().name}</div>
          <div class="device-sub">{d().connection}</div>
        </div>
        <span class={`pill ${statusTone(d())}`}>
          <span class="dot" />
          {statusText(d())}
        </span>
      </div>
      <div class="device-level">
        <span class="percent">
          <Show when={d().percent !== null} fallback={<span classList={{ muted: !d().level }}>{valueText(d()) ?? "—"}</span>}>
            {d().percent}
            <span class="unit">%</span>
          </Show>
        </span>
        <Show when={d().charging}>
          <span class="charging" title="Charging">
            <BoltIcon size={26} />
          </span>
        </Show>
      </div>
      <BatteryBar percent={barPercent(d())} dim={!live()} />
      <dl class="meta">
        <div>
          <dt>Updated</dt>
          <dd title={clock(d().lastUpdate)}>{relativeTime(d().lastUpdate, now())}</dd>
        </div>
        <div>
          <dt>Data</dt>
          <dd>{sourceText(d())}</dd>
        </div>
        <div>
          <Show
            when={d().discovered}
            fallback={
              <>
                <dt>Receiver</dt>
                <dd class="mono">{d().hardware ?? "Not found"}</dd>
              </>
            }
          >
            <dt>Connection</dt>
            <dd>{d().connection}</dd>
          </Show>
        </div>
      </dl>
      <Show when={d().status === "error" && d().error}>
        <p class="error-text">{d().error}</p>
      </Show>
    </article>
  );
}
