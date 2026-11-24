import { render } from "solid-js/web";
import { App } from "./App";
import { load } from "./app/store";
import "./styles/app.css";

// No browser context menu or text-selection chrome: this is a native-feeling utility window.
window.addEventListener("contextmenu", (e) => {
  if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
});

load().then(() => render(() => <App />, document.getElementById("root")!));
