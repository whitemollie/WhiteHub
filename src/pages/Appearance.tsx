import { For, Show } from "solid-js";
import { api, type Colors, type Colorize, type IconSet, type IndicatorMode, type ThemeMode } from "../app/api";
import { DEFAULTS } from "../app/format";
import { state, updateSettings } from "../app/store";
import { Button, ColorField, Row, Section, Segmented, Slider } from "../components/Controls";
import { DeviceGlyph } from "../components/DeviceCard";

const MAX_ICON_BYTES = 384 * 1024;

export function Appearance() {
  const a = () => state.settings!.appearance;
  const ind = () => state.settings!.indicators;
  const base = () => DEFAULTS[a().mode];
  const color = (key: keyof Colors, fallback: () => string, title: string) => (
    <Row title={title}>
      <ColorField label={title} value={a().colors[key]} fallback={fallback()} onChange={(v) => updateSettings((s) => (s.appearance.colors[key] = v))} />
    </Row>
  );
  const resetAppearance = async () => {
    const d = await api.defaultSettings();
    updateSettings((s) => {
      s.appearance = d.appearance;
      s.indicators = d.indicators;
    }, 0);
  };
  return (
    <div class="page">
      <header class="page-head">
        <div>
          <h1>Appearance</h1>
          <p class="muted">Theme, colours and the taskbar indicator.</p>
        </div>
        <Button variant="ghost" onClick={resetAppearance}>
          Reset
        </Button>
      </header>

      <Section title="Theme">
        <Row title="Mode">
          <Segmented<ThemeMode>
            label="Theme mode"
            value={a().mode}
            options={[
              { value: "dark", label: "Dark" },
              { value: "light", label: "Light" },
            ]}
            onChange={(v) => updateSettings((s) => (s.appearance.mode = v), 0)}
          />
        </Row>
        <Row title="Window transparency" hint="Below 100% the window uses the Windows acrylic backdrop. Applies when reopened.">
          <Slider label="Window opacity" min={60} max={100} step={5} value={Math.round(a().opacity * 100)} format={(v) => `${v}%`} onInput={(v) => updateSettings((s) => (s.appearance.opacity = v / 100))} />
        </Row>
      </Section>

      <Section title="Taskbar indicator">
        <Row title="Style" hint="Strip next to the tray, or one tray icon per device.">
          <Segmented<IndicatorMode>
            label="Indicator style"
            value={ind().mode}
            options={[
              { value: "taskbar", label: "Taskbar strip" },
              { value: "tray", label: "Tray icons" },
            ]}
            onChange={(v) => updateSettings((s) => (s.indicators.mode = v), 0)}
          />
        </Row>
        <Row title="Coloured levels">
          <Segmented<Colorize>
            label="Coloured levels"
            value={ind().colorize}
            options={[
              { value: "off", label: "Off" },
              { value: "lowOnly", label: "Low only" },
              { value: "all", label: "Always" },
            ]}
            onChange={(v) => updateSettings((s) => (s.indicators.colorize = v), 0)}
          />
        </Row>
        <Row title="Icons">
          <Segmented<IconSet>
            label="Icon set"
            value={a().iconSet}
            options={[
              { value: "fluent", label: "Headphones" },
              { value: "alternate", label: "Headset" },
              { value: "text", label: "Letters" },
            ]}
            onChange={(v) => updateSettings((s) => (s.appearance.iconSet = v), 0)}
          />
        </Row>
      </Section>

      <Section title="Battery levels">
        <Row title="High from" hint="At or above this level the indicator is green.">
          <Slider label="High threshold" min={40} max={95} step={5} value={a().highThreshold} format={(v) => `${v}%`} onInput={(v) => updateSettings((s) => (s.appearance.highThreshold = Math.max(v, s.appearance.lowThreshold + 5)))} />
        </Row>
        <Row title="Low below" hint="Below this level the indicator is red.">
          <Slider label="Low threshold" min={5} max={60} step={5} value={a().lowThreshold} format={(v) => `${v}%`} onInput={(v) => updateSettings((s) => (s.appearance.lowThreshold = Math.min(v, s.appearance.highThreshold - 5)))} />
        </Row>
        {color("high", () => DEFAULTS.high, "High colour")}
        {color("mid", () => DEFAULTS.mid, "Medium colour")}
        {color("low", () => DEFAULTS.low, "Low colour")}
      </Section>

      <Section title="Colours">
        {color("accent", () => base().accent, "Accent")}
        {color("background", () => base().background, "Background")}
        {color("card", () => base().card, "Cards")}
        {color("text", () => base().text, "Text")}
        {color("textMuted", () => base().textMuted, "Secondary text")}
      </Section>

      <Section title="Device icons">
        <For each={state.devices}>
          {(d) => {
            const custom = () => state.settings!.devices[d.id]?.customIcon;
            const pick = (file: File | undefined) => {
              if (!file) return;
              if (file.size > MAX_ICON_BYTES) return alert("Please choose an image smaller than 384 KB.");
              const r = new FileReader();
              r.onload = () => updateSettings((s) => (s.devices[d.id].customIcon = String(r.result)), 0);
              r.readAsDataURL(file);
            };
            return (
              <Row title={d.name} hint="PNG or SVG for the dashboard. The taskbar keeps the built-in icons for legibility.">
                <div class="icon-picker">
                  <span class="device-icon">
                    <DeviceGlyph device={d} size={20} />
                  </span>
                  <label class="btn">
                    Choose…
                    <input type="file" accept="image/png,image/svg+xml,image/jpeg,image/webp" hidden onChange={(e) => pick(e.currentTarget.files?.[0])} />
                  </label>
                  <Show when={custom()}>
                    <button type="button" class="link" onClick={() => updateSettings((s) => (s.devices[d.id].customIcon = null), 0)}>
                      Remove
                    </button>
                  </Show>
                </div>
              </Row>
            );
          }}
        </For>
      </Section>
    </div>
  );
}
