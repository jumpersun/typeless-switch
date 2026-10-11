use crate::{
    engine::{Client, Engine},
    files::{Captured, SessionFiles},
    vault::{SystemProtector, Vault},
    Result,
};
use std::{
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};
use sysinfo::System;

pub struct NativeClient {
    executable: PathBuf,
}
impl NativeClient {
    fn processes(&self) -> Vec<sysinfo::Pid> {
        let system = System::new_all();
        system
            .processes()
            .iter()
            .filter_map(|(id, p)| {
                let exe = p.exe()?;
                if same_path(exe, &self.executable) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }
}
fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}
impl Client for NativeClient {
    fn available(&self) -> bool {
        self.executable.is_file()
    }
    fn running(&self) -> bool {
        !self.processes().is_empty()
    }
    fn stop(&self) -> Result<()> {
        if !self.running() {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            // Ask the official app to quit; never force-kill an active recording.
            let status = Command::new("/usr/bin/osascript")
                .args(["-e", "tell application id \"now.typeless.desktop\" to quit"])
                .status()
                .map_err(|_| "无法请求 Typeless 退出。")?;
            if !status.success() {
                return Err("请结束录音并手动退出 Typeless，然后重试。".into());
            }
        }
        #[cfg(target_os = "windows")]
        for pid in self.processes() {
            use std::os::windows::process::CommandExt;
            // Only a numeric PID is interpolated. CloseMainWindow sends WM_CLOSE, never terminates.
            let script = format!("$p=Get-Process -Id {} -ErrorAction SilentlyContinue; if($p){{$null=$p.CloseMainWindow()}}", pid.as_u32());
            Command::new("powershell.exe")
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .status()
                .map_err(|_| "请手动退出 Typeless 后重试。")?;
        }
        let deadline = Instant::now() + Duration::from_secs(12);
        while self.running() {
            if Instant::now() >= deadline {
                return Err("Typeless 尚未退出，请结束录音并从托盘退出后重试。".into());
            }
            thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }
    fn start(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let app = self
                .executable
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .ok_or("无效的客户端路径")?;
            let status = Command::new("/usr/bin/open")
                .arg(app)
                .status()
                .map_err(|_| "无法启动 Typeless。")?;
            if !status.success() {
                return Err("无法启动 Typeless。".into());
            }
        }
        #[cfg(target_os = "windows")]
        {
            Command::new(&self.executable)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|_| "无法启动 Typeless。")?;
        }
        Ok(())
    }
    fn wait_for_account(&self, files: &SessionFiles, id: &str) -> Result<Captured> {
        let deadline = Instant::now() + Duration::from_secs(25);
        while Instant::now() < deadline {
            if let Ok(captured) = files.capture() {
                if captured.identity.user_id == id {
                    return Ok(captured);
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
        Err("Typeless 未确认目标账号，可能需要重新登录。".into())
    }
}
pub fn engine() -> Result<Engine> {
    let home = dirs::home_dir().ok_or("无法确定用户目录")?;
    #[cfg(target_os = "macos")]
    let (data, default_app) = (home.join("Library/Application Support/Typeless"), {
        let global = PathBuf::from("/Applications/Typeless.app/Contents/MacOS/Typeless");
        if global.exists() {
            global
        } else {
            home.join("Applications/Typeless.app/Contents/MacOS/Typeless")
        }
    });
    #[cfg(target_os = "windows")]
    let (data, default_app) = {
        let roaming = dirs::config_dir().ok_or("无法确定 AppData 目录")?;
        let local = dirs::data_local_dir().ok_or("无法确定 LocalAppData 目录")?;
        let candidates = [
            local.join("Programs/Typeless/Typeless.exe"),
            local.join("Typeless/Typeless.exe"),
            PathBuf::from(
                std::env::var_os("ProgramFiles").unwrap_or_else(|| "C:\\Program Files".into()),
            )
            .join("Typeless/Typeless.exe"),
        ];
        (
            roaming.join("Typeless"),
            candidates
                .iter()
                .find(|p| p.is_file())
                .cloned()
                .unwrap_or(candidates[0].clone()),
        )
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let (data, default_app) = (
        home.join(".config/Typeless"),
        PathBuf::from("/nonexistent/Typeless"),
    );
    let data = std::env::var_os("TYPELESS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or(data);
    let executable = std::env::var_os("TYPELESS_APP_PATH")
        .map(PathBuf::from)
        .unwrap_or(default_app);
    if !data.is_absolute() || !executable.is_absolute() {
        return Err("自定义 Typeless 路径必须是完整的绝对路径。".into());
    }
    let root = dirs::data_local_dir()
        .ok_or("无法确定账号库目录")?
        .join("io.typeless-switch.app");
    // Development-only isolation for native UI QA; release builds always use the system vault.
    #[cfg(debug_assertions)]
    let root = if let Some(path) = std::env::var_os("TYPELESS_TEST_VAULT_DIR") {
        let path = PathBuf::from(path);
        if !path.is_absolute() { return Err("测试账号库必须使用绝对路径。".into()); }
        path
    } else { root };
    Ok(Engine {
        files: SessionFiles {
            root: data,
            profiles: crate::codec::profiles(),
        },
        vault: Vault {
            root,
            protector: Box::new(SystemProtector),
        },
        client: Box::new(NativeClient { executable }),
    })
}
