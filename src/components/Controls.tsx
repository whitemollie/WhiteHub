import { For, Show, type JSX } from "solid-js";

export function Toggle(props: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      class="toggle"
      aria-checked={props.checked}
      aria-label={props.label}
      disabled={props.disabled}
      onClick={() => props.onChange(!props.checked)}
    >
      <span class="toggle-thumb" />
    </button>
  );
}

export function Segmented<T extends string | number>(props: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div class="segmented" role="radiogroup" aria-label={props.label}>
      <For each={props.options}>
        {(o) => (
          <button type="button" role="radio" aria-checked={props.value === o.value} classList={{ active: props.value === o.value }} onClick={() => props.onChange(o.value)}>
            {o.label}
          </button>
        )}
      </For>
    </div>
  );
}

/** A labelled settings row: title + optional hint on the left, control on the right. */
export function Row(props: { title: string; hint?: string; stack?: boolean; children: JSX.Element }) {
  return (
    <div class="row" classList={{ stack: props.stack }}>
      <div class="row-text">
        <div class="row-title">{props.title}</div>
        <Show when={props.hint}>
          <div class="row-hint">{props.hint}</div>
        </Show>
      </div>
      <div class="row-control">{props.children}</div>
    </div>
  );
}

export function Section(props: { title?: string; action?: JSX.Element; children: JSX.Element }) {
  return (
    <section class="section">
      <Show when={props.title || props.action}>
        <header class="section-head">
          <h2>{props.title}</h2>
          {props.action}
        </header>
      </Show>
      <div class="card">{props.children}</div>
    </section>
  );
}

export function Slider(props: { value: number; min: number; max: number; step?: number; onInput: (v: number) => void; label: string; format?: (v: number) => string }) {
  return (
    <div class="slider">
      <input
        type="range"
        aria-label={props.label}
        min={props.min}
        max={props.max}
        step={props.step ?? 1}
        value={props.value}
        style={{ "--p": `${((props.value - props.min) / (props.max - props.min)) * 100}%` }}
        onInput={(e) => props.onInput(Number(e.currentTarget.value))}
      />
      <span class="slider-value">{props.format ? props.format(props.value) : props.value}</span>
    </div>
  );
}

export function ColorField(props: { value: string | null; fallback: string; onChange: (v: string | null) => void; label: string }) {
  return (
    <div class="color-field">
      <label class="swatch" style={{ background: props.value ?? props.fallback }}>
        <input type="color" aria-label={props.label} value={props.value ?? props.fallback} onInput={(e) => props.onChange(e.currentTarget.value)} />
      </label>
      <span class="mono">{(props.value ?? props.fallback).toUpperCase()}</span>
      <button type="button" class="link" disabled={props.value === null} onClick={() => props.onChange(null)}>
        Reset
      </button>
    </div>
  );
}

export function Button(props: { onClick: () => void; children: JSX.Element; variant?: "primary" | "ghost" | "danger"; disabled?: boolean; title?: string }) {
  return (
    <button type="button" class={`btn ${props.variant ?? ""}`} disabled={props.disabled} title={props.title} onClick={props.onClick}>
      {props.children}
    </button>
  );
}
