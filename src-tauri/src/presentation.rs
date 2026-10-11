//! Apply the saved theme to the ordinary desktop window and its native title bar.
use crate::Result;
use serde::Deserialize;
use tauri::{Manager, Theme};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorTheme {
    System,
    Light,
    Dark,
}
#[derive(Deserialize)]
pub struct Preferences {
    theme: ColorTheme,
}

#[tauri::command]
pub fn presentation_update(app: tauri::AppHandle, preferences: Preferences) -> Result<()> {
    app.get_webview_window("main")
        .ok_or("窗口不存在。")?
        .set_theme(match preferences.theme {
            ColorTheme::System => None,
            ColorTheme::Light => Some(Theme::Light),
            ColorTheme::Dark => Some(Theme::Dark),
        })
        .map_err(|e| e.to_string())
}
