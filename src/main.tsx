/// <reference types="vite/client" />
import { t, locale } from "./i18n";
import React, { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { createPortal } from "react-dom";
import { invoke, isTauri } from "@tauri-apps/api/core";
import "./style.css";
import appIcon from "../src-tauri/icon.svg?raw";
import { usePreferences } from "./preferences";
import { Settings } from "./Settings";
import { listen } from "@tauri-apps/api/event";

type Account = { id: string; email: string; savedAt: number; expired: boolean };
type State = {
  accounts: Account[];
  current: Account | null;
  pending: string | null;
  installed: boolean;
  notice: string | null;
};
// Only the isolated UI preview build opts in on desktop; normal app URLs cannot enable mocks.
const preview =
  (!isTauri() || import.meta.env.VITE_UI_PREVIEW === "true") &&
  new URLSearchParams(location.search).has("preview");
const demo = (id: string, email: string, expired = false): Account => ({
  id,
  email,
  expired,
  savedAt: Date.now() / 1000,
});
let previewState: State = {
  accounts: [
    demo("personal", "hello@example.com"),
    demo("work", "work@example.com"),
    demo("old", "archive@example.com", true),
  ],
  current: demo("personal", "hello@example.com"),
  pending: null,
  installed: true,
  notice: null,
};
const initialMode = new URLSearchParams(location.search).get("preview");
if (initialMode === "empty")
  previewState = {
    ...previewState,
    accounts: [],
    current: null,
    notice: "请先在 Typeless 中登录账号。",
  };
if (initialMode === "recover") previewState.pending = "switch";
async function call(action?: string, id?: string): Promise<State> {
  if (preview) {
    await new Promise((resolve) => setTimeout(resolve, 320));
    if (action === "remove")
      previewState.accounts = previewState.accounts.filter((a) => a.id !== id);
    if (action === "switch")
      previewState.current =
        previewState.accounts.find((a) => a.id === id) ?? null;
    if (action === "add") previewState.pending = "add";
    if (action === "restore") previewState.pending = null;
    if (action === "finish_add") {
      previewState.pending = null;
      const account = demo("new", "new@example.com");
      previewState.accounts = [
        ...previewState.accounts.filter((a) => a.id !== "new"),
        account,
      ];
      previewState.current = account;
    }
    if (
      action === "save" &&
      previewState.current &&
      !previewState.accounts.some((a) => a.id === previewState.current?.id)
    )
      previewState.accounts.push(previewState.current);
    return structuredClone(previewState);
  }
  if (!isTauri()) throw new Error("请打开桌面应用。此网页无法访问本机账号。");
  return action
    ? invoke<State>("account_action", { action, id: id ?? null })
    : invoke<State>("account_state");
}
function Glyph() {
  // The trusted local SVG stays vector; CSS strips tile/shadow effects for this small mark.
  return <span className="app-icon" aria-hidden="true" dangerouslySetInnerHTML={{ __html: appIcon }} />;
}
type IconName = "plus" | "check" | "lock" | "trash" | "arrow" | "save";
function Icon({ name }: { name: IconName }) {
  const paths: Record<IconName, React.ReactNode> = {
    plus: <path d="M12 5v14M5 12h14" />,
    check: <path d="m5 12 4 4L19 6" />,
    lock: (
      <>
        <rect x="5" y="10" width="14" height="11" rx="3" />
        <path d="M8 10V7a4 4 0 0 1 8 0v3M12 14v3" />
      </>
    ),
    trash: (
      <>
        <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v5M14 11v5" />
      </>
    ),
    arrow: <path d="M5 12h14m-5-5 5 5-5 5" />,
    save: (
      <>
        <path d="M12 3v11m-4-4 4 4 4-4M5 15v5h14v-5" />
      </>
    ),
  };
  return (
    <svg
      className="symbol"
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name]}
    </svg>
  );
}
function Avatar({ email, large = false }: { email: string; large?: boolean }) {
  const letter = Array.from(email.trim())[0]?.toLocaleUpperCase() ?? "";
  return <span className={`avatar${large ? " large" : ""}`} aria-hidden="true">{letter}</span>;
}
function SettingsIcon() { return <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="m9 3-.6 2-2 .9-2-.5-2 3.4 1.5 1.5v2.4L2.4 14l2 3.4 2-.5 2 .9.6 2h4l.6-2 2-.9 2 .5 2-3.4-1.5-1.5v-2.4l1.5-1.5-2-3.4-2 .5-2-.9-.6-2Z"/></svg>; }
function RefreshIcon() { return <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            aria-hidden="true"
          >
            <path
              d="M20 7v5h-5M4 17v-5h5M6.1 7a7 7 0 0 1 11.8-1L20 9M4 15l2.1 3A7 7 0 0 0 17.9 17"
              stroke="currentColor"
              strokeWidth="1.7"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>; }
function App() {
  const preferences = usePreferences();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [presentationError, setPresentationError] = useState("");
  const [refreshedAt, setRefreshedAt] = useState<number | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    invoke("presentation_update", { preferences }).then(() => { if (active) setPresentationError(""); }).catch(() => { if (active) setPresentationError("桌面窗口设置未能应用，请重新打开应用。"); });
    return () => { active = false; };
  }, [preferences]);
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    for (const [event, callback] of [
      ["exit-blocked", () => setPresentationError("操作进行中，请完成后再退出。")],
    ] as const) void listen(event, callback).then((unlisten) => { if (disposed) unlisten(); else unlisteners.push(unlisten); }).catch(() => { if (!disposed) setPresentationError("桌面窗口设置未能应用，请重新打开应用。"); });
    return () => { disposed = true; unlisteners.forEach((unlisten) => unlisten()); };
  }, []);
  const [state, setState] = useState<State | null>(null);
  const [busy, setBusy] = useState<string | null>("load");
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [dialog, setDialog] = useState<{
    type: "add" | "remove";
    account?: Account;
  } | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const working = useRef(false);
  const run = async (action?: string, id?: string) => {
    if (working.current) return;
    working.current = true;
    setBusy(action ?? "load");
    setError("");
    setMessage("");
    try {
      const next = await call(action, id);
      setState(next);
      setRefreshedAt(Date.now());
      if (action && next.notice) setError(next.notice);
      if (action && !next.notice)
        setMessage(
          (
            {
              save: "当前账号已保存。",
              switch: "已切换，请在 Typeless 中确认可正常使用。",
              remove: "已从本机账号库移除。",
              finish_add: "账号已添加。",
              restore: "已恢复原登录状态。",
            } as Record<string, string>
          )[action] ?? "",
        );
    } catch (e) {
      setError(
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : "操作失败，请重试。",
      );
      try {
        setState(await call());
      } catch {
        /* Preserve last view if the vault cannot be read. */
      }
    } finally {
      working.current = false;
      setBusy(null);
      setPresentationError((previous) => previous === "操作进行中，请完成后再退出。" ? "" : previous);
    }
  };
  useEffect(() => {
    void run();
  }, []);
  useEffect(() => {
    if (dialog) dialogRef.current?.showModal();
  }, [dialog]);
  const locked = Boolean(busy || state?.pending);
  const currentSaved = state?.accounts.some((a) => a.id === state.current?.id);
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label={t("应用导航")}>
        <div className="brand">
          <span className="logo"><Glyph /></span>
          <span className="brand-name">Typeless <span>Switch</span></span>
        </div>
        <nav aria-label={t("主导航")}>
          <a className="nav-item selected" href="#accounts-page" aria-current="page" aria-label={t("账号")}>
            <Icon name="arrow" /><span>{t("账号")}</span>
          </a>
        </nav>
        <div className="sidebar-bottom">
          <div className="local-note"><Icon name="lock" /><p>{t("账号加密保存在此设备")}</p></div>
          <button className="nav-item settings-button" onClick={() => setSettingsOpen(true)} title={t("设置")} aria-label={t("设置")}>
            <SettingsIcon /><span>{t("设置")}</span>
          </button>
          <span className="version">v0.1.0</span>
        </div>
      </aside>
      <main id="accounts-page" aria-busy={Boolean(busy)} tabIndex={-1}>
      {preview && <div className="preview">{t("界面预览 · 使用示例账号，不会修改 Typeless")}</div>}
      <header className="page-heading">
        <div><h1>{t("账号")}</h1><p>{t("你的账号，一处管理。")}</p></div>
        <div className="header-actions">
          <button className="icon-button refresh" disabled={Boolean(busy)} onClick={() => void run()} title={t("刷新账号状态")} aria-label={t("刷新账号状态")}>
            <RefreshIcon />
          </button>
          <button className="primary" disabled={locked || !state?.installed} onClick={() => setDialog({ type: "add" })}>
            <Icon name="plus" />{t("添加账号")}
          </button>
        </div>
      </header>
      {presentationError && <div className="notice error" role="alert">{t(presentationError)}</div>}
      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      {error && (
        <div className="notice error" role="alert">
          {t(error)}
        </div>
      )}
      {message && (
        <div className="notice success" role="status">
          {t(message)}
        </div>
      )}
      {state?.pending && (
        <section className="recovery" aria-labelledby="pending-title">
          <span className="eyebrow">
            {state.pending === "add" ? t("等待登录") : t("操作恢复")}
          </span>
          <h2 id="pending-title">
            {state.pending === "add"
              ? t("在 Typeless 中登录另一个账号")
              : t("上一次切换尚未完成")}
          </h2>
          <p>
            {state.pending === "add"
              ? t("在官方窗口使用邮箱完成注册或登录，并按提示验证。完成后回到这里，检测并保存账号。原账号已备份，可随时恢复。")
              : t("原登录状态的备份已保留。恢复后即可继续管理账号。")}
          </p>
          <div className="actions">
            {state.pending === "add" && (
              <button
                className="primary"
                disabled={Boolean(busy)}
                onClick={() => void run("finish_add")}
              >{t("我已登录，检测并保存")}</button>
            )}
            <button
              disabled={Boolean(busy)}
              onClick={() => void run("restore")}
            >{t("恢复原账号")}</button>
          </div>
        </section>
      )}
      <div>
        <section className="current" aria-labelledby="current-title">
          <div className="current-top">
            <span className="eyebrow" id="current-title">{t("当前账号")}</span>
            <span className={`status ${state?.current && !state.current.expired ? "" : "muted"}`}>
              <i />
              {state?.current?.expired ? t("登录已过期") : state?.current ? t("已登录") : t("未登录")}
            </span>
          </div>
          <div className="current-body">
            {state?.current && <Avatar email={state.current.email} large />}
            <div className="identity">
              <h2>
                {state?.current?.email ??
                  (busy === "load" ? t("正在读取…") : t("从第一个账号开始"))}
              </h2>
              <p>
                {state?.current?.expired ? t("请在 Typeless 中重新登录后更新会话。") : state?.current
                  ? currentSaved
                    ? t("登录状态已加密保存在此设备")
                    : t("保存后，下次可以直接切换回来")
                  : (state?.notice ? t(state.notice) :
                    t("打开 Typeless 并完成登录，再保存到这里。"))}
              </p>
            </div>
          </div>
          <div className="current-actions">
          <button
            className="save"
            disabled={locked || !state?.current || state.current.expired}
            onClick={() => void run("save")}
          >
            {busy === "save"
              ? t("正在保存…")
              : currentSaved
                ? t("更新登录状态")
                : t("保存当前账号")}
            <Icon name={currentSaved ? "check" : "save"} />
          </button>
          </div>
        </section>
        <p className="switch-hint">{t("切换会重启 Typeless，请先结束录音。")}</p>
        <section className="accounts" aria-labelledby="accounts-title">
          <div className="section-top">
            <h2 id="accounts-title">{t("其他账号")}<span>{state?.accounts.filter(a => a.id !== state.current?.id).length ?? 0}</span>
            </h2>
          </div>
          {!state?.accounts.some(a => a.id !== state.current?.id) ? (
            <div className="empty">
              <h3>{state?.current ? t("还没有其他账号") : t("账号都放在这里")}</h3>
              <p>{state?.current ? t("添加另一个账号后，就能在这里切换。") : t("先保存当前账号，再添加另一个。")}</p>
            </div>
          ) : (
            <ul>
              {state.accounts.filter(account => account.id !== state.current?.id).map((account) => {
                return (
                  <li key={account.id} className="account-row">
                    <Avatar email={account.email} />
                    <div className="identity">
                      <h3>{account.email}</h3>
                      <p>
                        {account.expired
                          ? t("保存的会话已过期 · 请重新添加")
                          : t("保存在此设备")}
                      </p>
                    </div>
                    <button
                      className="switch"
                      disabled={locked || account.expired}
                      onClick={() => void run("switch", account.id)}
                    >
                      {t("切换")}
                      <Icon name="arrow" />
                    </button>
                    <button
                      className="remove icon-button"
                      disabled={locked}
                      title={t("移除 {0}", [account.email])}
                      aria-label={t("移除 {0}", [account.email])}
                      onClick={() => setDialog({ type: "remove", account })}
                    >
                      <Icon name="trash" />
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
        {state && !state.installed && (
          <div className="notice">{t("未找到 Typeless，请先安装官方客户端。自定义安装路径可在 README 中查看。")}</div>
        )}
      </div>
      <footer>
        <span>{refreshedAt ? t("更新于 {0}", [new Date(refreshedAt).toLocaleTimeString(locale())]) : t("正在读取…")}</span>
      </footer>
      {busy &&
        busy !== "load" &&
        createPortal(
          <div className="busy" role="status">
            <span className="spinner" />
            {busy === "switch"
              ? t("正在切换并等待 Typeless 确认…")
              : busy === "add"
                ? t("正在打开 Typeless 登录窗口…")
                : busy === "restore"
                  ? t("正在恢复原账号…")
                  : t("正在处理…")}
          </div>,
          document.body,
        )}
      <dialog
        ref={dialogRef}
        className={dialog?.type === "add" ? "add-dialog" : undefined}
        aria-labelledby="account-dialog-title"
        onCancel={() => setDialog(null)}
        onClose={() => setDialog(null)}
      >
        <h2 id="account-dialog-title">
          {dialog?.type === "add" ? t("添加另一个账号") : t("移除保存的账号？")}
        </h2>
        <p>
          {dialog?.type === "add"
            ? t("使用你已有的邮箱，在 Typeless 官方窗口完成注册或登录。")
            : t("将从本机账号库移除 {0}，不会注销 Typeless 当前账号。", [dialog?.account?.email ?? ""])}
        </p>
        {dialog?.type === "add" && (
          <>
            <ol className="signup-steps">
              <li>{t("打开 Typeless，使用邮箱注册或登录。")}</li>
              <li>{t("按官方提示完成邮箱验证。")}</li>
              <li>{t("回到这里，点击「我已登录，检测并保存」。")}</li>
            </ol>
            <p className="restart-note">{t("请先结束录音。工具会备份当前登录状态并重启 Typeless。")}</p>
          </>
        )}
        <div className="actions dialog-actions">
          <button
            onClick={() => {
              dialogRef.current?.close();
              setDialog(null);
            }}
          >{t("取消")}</button>
          <button
            className="primary"
            onClick={() => {
              const choice = dialog;
              dialogRef.current?.close();
              setDialog(null);
              void run(
                choice?.type === "add" ? "add" : "remove",
                choice?.account?.id,
              );
            }}
          >
            {dialog?.type === "add" ? t("打开 Typeless") : t("移除账号")}
          </button>
        </div>
      </dialog>
      </main>
    </div>
  );
}
const root = createRoot(document.getElementById("root")!);
root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
// This file owns the root; release it before Vite re-evaluates the entry.
if (import.meta.hot) import.meta.hot.dispose(() => root.unmount());
