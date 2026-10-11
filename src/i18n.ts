import { english } from "./translations";
import { getPreferences } from "./preferences";
const normalize = (value: string) => value.replace(/\s+/g, " ").trim();
const reverse = new Map(Object.entries(english).map(([zh, en]) => [normalize(en), zh]));
const patterns = Object.entries(english).filter(([key]) => /\{\d+\}/.test(key)).map(([zh, en]) => {
  const compile = (value: string) => new RegExp("^" + normalize(value).split(/(\{\d+\})/).map((part) => /^\{\d+\}$/.test(part) ? "(.*?)" : part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("") + "$", "u");
  return { zh, en, zhPattern: compile(zh), enPattern: compile(en) };
});
function format(template: string, values: unknown[]) { return template.replace(/\{(\d+)\}/g, (_, index: string) => String(values[Number(index)] ?? "")); }
export function t(source: string, values?: unknown[], depth = 0): string {
  const key = normalize(source);
  const en = getPreferences().language === "en";
  if (values) return format(en ? english[key] ?? source : source, values);
  if (english[key] !== undefined) return en ? english[key] : source;
  if (reverse.has(key)) return en ? source : reverse.get(key)!;
  for (const pattern of patterns) {
    const match = (en ? pattern.zhPattern : pattern.enPattern).exec(key);
    if (match) return format(en ? pattern.en : pattern.zh, match.slice(1).map(value => depth < 3 ? t(value, undefined, depth + 1) : value));
  }
  return source;
}
export const locale = () => getPreferences().language === "zh" ? "zh-CN" : "en-US";
