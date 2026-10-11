import { useSyncExternalStore } from "react";
import { PREFERENCES_KEY, readPreferences, resolvedTheme, type Preferences } from "./preferences-model";
export type { Preferences } from "./preferences-model";
let stored: string | null = null;
try { stored = localStorage.getItem(PREFERENCES_KEY); } catch { /* Preferences remain usable in restricted webviews. */ }
let preferences = readPreferences(stored, navigator.language);
const listeners = new Set<() => void>();
const system = matchMedia("(prefers-color-scheme: dark)");
function apply() {
  document.documentElement.dataset.theme = resolvedTheme(preferences.theme, system.matches);

  document.documentElement.lang = preferences.language === "zh" ? "zh-CN" : "en";
}
function notify() { apply(); listeners.forEach((listener) => listener()); }
apply();
system.addEventListener("change", apply);
window.addEventListener("storage", (event) => {
  if (event.key === PREFERENCES_KEY) { preferences = readPreferences(event.newValue, navigator.language); notify(); }
});
export const getPreferences = () => preferences;
export function updatePreferences(patch: Partial<Preferences>) {
  const next = readPreferences(JSON.stringify({ ...preferences, ...patch }));
  // Keep UI and persistence in agreement; surface storage failures to the settings panel.
  localStorage.setItem(PREFERENCES_KEY, JSON.stringify(next));
  preferences = next; notify();
}
export function usePreferences() {
  return useSyncExternalStore((listener) => { listeners.add(listener); return () => { listeners.delete(listener); }; }, getPreferences);
}
