import { initI18n } from "$lib/i18n";
import { waitLocale } from "svelte-i18n";
import { mount } from "svelte";
import App from "./App.svelte";

initI18n();

waitLocale().then(() => {
  mount(App, {
    target: document.getElementById("app")!,
  });
});
