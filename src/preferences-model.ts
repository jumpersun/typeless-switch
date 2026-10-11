export type Preferences = { language: "zh" | "en"; theme: "system" | "light" | "dark" };
export const PREFERENCES_KEY = "typeless-switch.preferences.v1";
export function readPreferences(raw: string | null, language = "zh"): Preferences {
  let value: Partial<Preferences> = {};
  try { const parsed: unknown = JSON.parse(raw ?? "{}"); if (parsed && typeof parsed === "object") value = parsed; } catch { /* A damaged preference never prevents opening the account vault. */ }
  return {
    language: value.language === "en" || value.language === "zh" ? value.language : language.toLowerCase().startsWith("zh") ? "zh" : "en",
    theme: value.theme === "dark" || value.theme === "light" ? value.theme : "system",
  };
}
export function resolvedTheme(theme: Preferences["theme"], systemDark: boolean) {
  return theme === "system" ? systemDark ? "dark" : "light" : theme;
}
