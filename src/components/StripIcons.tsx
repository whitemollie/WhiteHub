// Previews of the taskbar strip. Shapes use the coordinates of src-tauri/src/shell/icons.rs.
import { For, Match, Show, Switch } from "solid-js";
import type { ChargeMark, Device, DeviceKind, IconSet } from "../app/api";
import { barPercent, levelColor, valueText } from "../app/format";
import { deviceSettings, state } from "../app/store";

const GLYPHS: Record<DeviceKind, string> = { mouse: "", keyboard: "", headset: "", gamepad: "", other: "" };
const HEADSET_GLYPH = "";
const LETTERS: Record<DeviceKind, string> = { mouse: "M", keyboard: "K", headset: "H", gamepad: "G", other: "B" };
const RING = 2 * Math.PI * 5.8;

export const ICON_SETS: { value: IconSet; label: string }[] = [
  { value: "fluent", label: "Outline" },
  { value: "alternate", label: "Headset" },
  { value: "filled", label: "Filled" },
  { value: "battery", label: "Battery" },
  { value: "ring", label: "Ring" },
  { value: "text", label: "Letters" },
  { value: "hidden", label: "Number only" },
];

export const CHARGE_MARKS: { value: ChargeMark; label: string }[] = [
  { value: "bolt", label: "Bolt" },
  { value: "boltOutline", label: "Thin bolt" },
  { value: "plug", label: "Plug" },
  { value: "plus", label: "Plus" },
  { value: "hidden", label: "None" },
];

function FilledShape(props: { kind: DeviceKind }) {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="currentColor" fill-rule="evenodd">
      <Switch>
        <Match when={props.kind === "mouse"}>
          <path d="M7.4 1v5.2H3v-.8A4.4 4.4 0 0 1 7.4 1zM8.6 1A4.4 4.4 0 0 1 13 5.4v.8H8.6zM3 7.4h10v3.2A4.4 4.4 0 0 1 8.6 15H7.4A4.4 4.4 0 0 1 3 10.6z" />
        </Match>
        <Match when={props.kind === "headset"}>
          <path d="M2 12V8a6 6 0 0 1 12 0v4" fill="none" stroke="currentColor" stroke-width="2" />
          <rect x="1" y="8.6" width="4.6" height="6.2" rx="1.6" />
          <rect x="10.4" y="8.6" width="4.6" height="6.2" rx="1.6" />
        </Match>
        <Match when={props.kind === "keyboard"}>
          <path d="M2.4 3.2h11.2a2 2 0 0 1 2 2v5.6a2 2 0 0 1-2 2H2.4a2 2 0 0 1-2-2V5.2a2 2 0 0 1 2-2zm.2 2.4v1.6h1.6V5.6zm3 0v1.6h1.6V5.6zm3 0v1.6h1.6V5.6zm3 0v1.6h1.6V5.6zm-7 3.6v1.5h6.8V9.2z" />
        </Match>
        <Match when={props.kind === "gamepad"}>
          <path d="M5 3.4h6a4.4 4.4 0 0 1 4.4 4.4v.4a4.4 4.4 0 0 1-4.4 4.4H5A4.4 4.4 0 0 1 .6 8.2v-.4A4.4 4.4 0 0 1 5 3.4zM4.7 6v1.3H3.4v1.2h1.3v1.3h1.2V8.5h1.3V7.3H5.9V6zm5.7 2.2a.8.8 0 1 0 0 1.6.8.8 0 0 0 0-1.6zm1.8-2.1a.8.8 0 1 0 0 1.6.8.8 0 0 0 0-1.6z" />
        </Match>
        <Match when={props.kind === "other"}>
          <rect x="2.4" y=".8" width="11.2" height="14.4" rx="5.6" />
          <path d="m5.4 5.5 5 4.9-2.4 2.3V3.3l2.4 2.3-5 4.9" fill="none" stroke="var(--strip-bg)" stroke-width="1.2" />
        </Match>
      </Switch>
    </svg>
  );
}

/** Icon of a device as the strip shows it. */
export function StripIcon(props: { set: IconSet; kind: DeviceKind; percent: number | null; color?: string }) {
  const percent = () => Math.min(100, Math.max(0, props.percent ?? 0));
  const fill = () => props.color ?? "currentColor";
  return (
    <Switch>
      <Match when={props.set === "fluent" || props.set === "alternate"}>
        <span class="strip-glyph">{props.set === "alternate" && props.kind === "headset" ? HEADSET_GLYPH : GLYPHS[props.kind]}</span>
      </Match>
      <Match when={props.set === "filled"}>
        <FilledShape kind={props.kind} />
      </Match>
      <Match when={props.set === "battery"}>
        <svg width="20" height="15" viewBox="0 0 21 16" fill="currentColor">
          <rect x=".8" y="3.6" width="17" height="8.8" rx="2.4" fill="none" stroke="currentColor" stroke-width="1.5" />
          <rect x="18.8" y="6.2" width="1.8" height="3.6" rx=".8" />
          <Show when={percent() > 0}>
            <rect x="2.6" y="5.4" width={Math.max(1.2, (13.4 * percent()) / 100)} height="5.2" rx=".6" fill={fill()} />
          </Show>
        </svg>
      </Match>
      <Match when={props.set === "ring"}>
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke-width="2.4">
          <circle cx="8" cy="8" r="5.8" stroke="currentColor" stroke-opacity=".28" />
          <circle cx="8" cy="8" r="5.8" stroke={fill()} stroke-dasharray={`${(RING * percent()) / 100} ${RING}`} transform="rotate(-90 8 8)" />
        </svg>
      </Match>
      <Match when={props.set === "text"}>
        <span class="strip-letter">{LETTERS[props.kind]}</span>
      </Match>
    </Switch>
  );
}

export function ChargeIcon(props: { mark: ChargeMark }) {
  const bolt = "M5.8 1.5.2 9.1h3.4L2.3 14.6l5.5-7.6H4.5z";
  return (
    <Switch>
      <Match when={props.mark === "bolt"}>
        <svg width="8" height="13" viewBox="0 1.4 8 13.3">
          <path d={bolt} fill="currentColor" />
        </svg>
      </Match>
      <Match when={props.mark === "boltOutline"}>
        <svg width="9" height="13" viewBox="-.8 1 9.6 14.1">
          <path d={bolt} fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
        </svg>
      </Match>
      <Match when={props.mark === "plug"}>
        <svg width="12" height="15" viewBox="0 0 12 16" fill="currentColor">
          <path d="M2.8 1.5h1.6v4h3.2v-4h1.6v4h1.4v2.2A3.6 3.6 0 0 1 7 11.3h-.2v3.3H5.2v-3.3H5a3.6 3.6 0 0 1-3.6-3.6V5.5h1.4z" />
        </svg>
      </Match>
      <Match when={props.mark === "plus"}>
        <svg width="10" height="15" viewBox="0 0 10 16" fill="currentColor">
          <path d="M3.9 4h2.2v2.9H9v2.2H6.1V12H3.9V9.1H1V6.9h2.9z" />
        </svg>
      </Match>
    </Switch>
  );
}

const example = (id: string, kind: DeviceKind, percent: number) => ({ id, kind, percent, level: null, charging: false, status: "online" }) as Device;
const EXAMPLES = [example("", "mouse", 61), example("", "headset", 47)];

/** The strip with the current settings; example devices while none is known. */
export function StripPreview() {
  const items = () => {
    const shown = state.devices.filter((d) => d.known && d.enabled && deviceSettings(d.id)?.showInIndicator && !(d.discovered && d.status === "disconnected"));
    return shown.length ? shown : EXAMPLES;
  };
  const settings = () => state.settings!;
  const color = (d: Device) => {
    const p = barPercent(d);
    const mode = settings().indicators.colorize;
    if (p === null || mode === "off" || (mode === "lowOnly" && p >= settings().appearance.lowThreshold)) return undefined;
    return levelColor(settings(), p);
  };
  return (
    <div class="strip-preview" aria-label="Preview of the taskbar strip">
      <For each={items()}>
        {(d, i) => (
          <span class="strip-item" classList={{ dim: d.status !== "online" }}>
            <StripIcon set={deviceSettings(d.id)?.iconSet ?? settings().appearance.iconSet} kind={d.kind} percent={barPercent(d)} color={color(d)} />
            <span class="strip-value" style={{ color: color(d) }}>
              {valueText(d) ?? "--"}
            </span>
            {/* Without a charging device the first one shows how the mark looks. */}
            <Show when={d.charging || (i() === 0 && !items().some((x) => x.charging))}>
              <ChargeIcon mark={settings().indicators.chargeMark} />
            </Show>
          </span>
        )}
      </For>
    </div>
  );
}
