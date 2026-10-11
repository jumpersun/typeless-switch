import { useEffect, useRef, useState } from "react";
import { t } from "./i18n";
import { updatePreferences, usePreferences, type Preferences } from "./preferences";
export function Settings({ open, onClose }: { open: boolean; onClose: () => void }) {
  const preferences = usePreferences();
  const dialog = useRef<HTMLDialogElement>(null);
  const [error, setError] = useState("");
  useEffect(() => { if (open) dialog.current?.showModal(); else dialog.current?.close(); }, [open]);
  function choose(patch: Partial<Preferences>) {
    try { updatePreferences(patch); setError(""); } catch { setError("无法保存设置，请检查本机存储权限。"); }
  }
  return <dialog ref={dialog} className="settings-dialog" aria-labelledby="settings-title" onCancel={onClose} onClose={onClose}>
    <div className="settings-heading"><div><h2 id="settings-title">{t("设置")}</h2><p>{t("按你的习惯使用 Typeless Switch。")}</p></div>
      <button className="icon-button" onClick={onClose} aria-label={t("关闭设置")}>×</button></div>
    <fieldset><legend>{t("主题")}</legend><div className="preference-segments">
      {(["light", "dark", "system"] as const).map((theme) => <button key={theme} aria-pressed={preferences.theme === theme} onClick={() => choose({ theme })}>{t({ light: "浅色", dark: "深色", system: "跟随系统" }[theme])}</button>)}
    </div></fieldset>
    <fieldset><legend>{t("语言")}</legend><div className="preference-segments">
      <button lang="zh-CN" aria-pressed={preferences.language === "zh"} onClick={() => choose({ language: "zh" })}>中文</button>
      <button lang="en" aria-pressed={preferences.language === "en"} onClick={() => choose({ language: "en" })}>English</button>
    </div></fieldset>
    <p className="settings-note">{t("设置自动保存在此设备。关闭窗口即退出应用。")}</p>
    {error && <p className="notice error" role="alert">{t(error)}</p>}
    <div className="actions"><button className="primary" onClick={onClose}>{t("完成")}</button></div>
  </dialog>;
}
