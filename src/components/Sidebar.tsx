import { For, type JSX } from "solid-js";
import { navigate, page, type Page } from "../app/store";
import { DevicesIcon, GridIcon, InfoIcon, PaletteIcon, SlidersIcon } from "./Icons";

const ITEMS: { id: Page; label: string; icon: () => JSX.Element }[] = [
  { id: "dashboard", label: "Dashboard", icon: () => <GridIcon /> },
  { id: "devices", label: "Devices", icon: () => <DevicesIcon /> },
  { id: "appearance", label: "Appearance", icon: () => <PaletteIcon /> },
  { id: "settings", label: "Settings", icon: () => <SlidersIcon /> },
  { id: "about", label: "About", icon: () => <InfoIcon /> },
];

export function Sidebar() {
  return (
    <nav class="sidebar" aria-label="Sections">
      <For each={ITEMS}>
        {(it) => (
          <button type="button" class="nav-item" classList={{ active: page() === it.id }} aria-current={page() === it.id ? "page" : undefined} onClick={() => navigate(it.id)}>
            {it.icon()}
            <span>{it.label}</span>
          </button>
        )}
      </For>
    </nav>
  );
}
