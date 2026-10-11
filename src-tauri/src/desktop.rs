use crate::{
    engine::{Engine, View},
    Result,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tauri::{Emitter, Manager};
#[derive(Clone)]
struct AppState {
    engine: Arc<Mutex<Engine>>,
    active_operations: Arc<AtomicUsize>,
}

// Keep queued and running mutations alive until their journal is committed or restored.
struct ActiveOperation(Arc<AtomicUsize>);
impl ActiveOperation {
    fn begin(state: &AppState) -> Self {
        state.active_operations.fetch_add(1, Ordering::SeqCst);
        Self(state.active_operations.clone())
    }
}
impl Drop for ActiveOperation {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
#[tauri::command]
async fn account_state(state: tauri::State<'_, AppState>) -> Result<View> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .engine
            .lock()
            .map_err(|_| "操作未完成，请重新打开工具。".to_string())?
            .view()
    })
    .await
    .map_err(|_| "读取账号失败".to_string())?
}
#[tauri::command]
async fn account_action(
    state: tauri::State<'_, AppState>,
    action: String,
    id: Option<String>,
) -> Result<View> {
    let state = state.inner().clone();
    let operation = ActiveOperation::begin(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        state
            .engine
            .lock()
            .map_err(|_| "操作未完成，请重新打开工具。".to_string())?
            .execute(&action, id.as_deref())
    })
    .await
    .map_err(|_| "操作中断，请重新打开工具恢复。".to_string())?
}
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState {
                engine: Arc::new(Mutex::new(crate::platform::engine()?)),
                active_operations: Arc::new(AtomicUsize::new(0)),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if operation_in_progress(window.app_handle()) {
                    api.prevent_close();
                    notify_busy(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            account_state,
            account_action,
            crate::presentation::presentation_update
        ])
        .build(tauri::generate_context!())
        .expect("无法启动 Typeless Switch")
        .run(|app, event| {
            // This also covers Cmd-Q / the native Quit menu while a switch is in flight.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if operation_in_progress(app) {
                    api.prevent_exit();
                    notify_busy(app);
                }
            }
        });
}

fn operation_in_progress(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>()
        .map(|state| state.active_operations.load(Ordering::SeqCst) > 0)
        .unwrap_or(false)
}

fn notify_busy(app: &tauri::AppHandle) {
    let _ = app.emit("exit-blocked", ());
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
