use crate::{
    codec::{self, Identity},
    Result,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub session: Option<Vec<u8>>,
    pub storage: Option<Vec<u8>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Captured {
    pub identity: Identity,
    pub session: Vec<u8>,
    pub profile: String,
}

pub struct SessionFiles {
    pub root: PathBuf,
    pub profiles: Vec<String>,
}

impl SessionFiles {
    pub fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            session: read_optional(&self.root.join("user-data.json"))?,
            storage: read_optional(&self.root.join("app-storage.json"))?,
        })
    }

    pub fn capture(&self) -> Result<Captured> {
        let session = read_optional(&self.root.join("user-data.json"))?
            .ok_or("请先在 Typeless 中登录账号。")?;
        let (identity, profile) = codec::inspect(&session, &self.profiles)?;
        let storage = read_optional(&self.root.join("app-storage.json"))?
            .ok_or("Typeless 尚未完成账号同步，请稍后重试。")?;
        let object: serde_json::Value = serde_json::from_slice(&storage)
            .map_err(|_| "账号缓存不可读，请打开 Typeless 后重试。")?;
        let user = &object["userData"];
        if user["user_id"].as_str() != Some(identity.user_id.as_str())
            || !user["email"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case(&identity.email))
        {
            return Err("Typeless 正在更新登录状态，请等待账号页显示正确邮箱后重试。".into());
        }
        if read_optional(&self.root.join("user-data.json"))?.as_ref() != Some(&session) {
            return Err("登录状态刚刚更新，请重试。".into());
        }
        Ok(Captured {
            identity,
            session,
            profile,
        })
    }

    pub fn apply(&self, target: Option<&Captured>) -> Result<()> {
        if let Some(target) = target {
            let (identity, _) = codec::inspect(&target.session, &self.profiles)?;
            if identity != target.identity {
                return Err("保存的账号身份不一致，请重新保存。".into());
            }
        }
        // Preserve shared settings. The official app must populate its own account cache again.
        let storage_path = self.root.join("app-storage.json");
        if let Some(bytes) = read_optional(&storage_path)? {
            let mut storage: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| "账号缓存损坏，已停止切换。")?;
            let map = storage.as_object_mut().ok_or("账号缓存格式不受支持。")?;
            for key in ["userData", "quotaUsage", "session", "currentRoute"] {
                map.remove(key);
            }
            atomic_write(
                &storage_path,
                &serde_json::to_vec(&storage).map_err(|_| "无法准备账号缓存")?,
            )?;
        }
        let session = self.root.join("user-data.json");
        match target {
            Some(target) => atomic_write(&session, &target.session),
            None => remove_optional(&session),
        }
    }

    pub fn restore(&self, snapshot: &Snapshot) -> Result<()> {
        for (name, data) in [
            ("app-storage.json", &snapshot.storage),
            ("user-data.json", &snapshot.session),
        ] {
            let path = self.root.join(name);
            match data {
                Some(bytes) => atomic_write(&path, bytes)?,
                None => remove_optional(&path)?,
            }
        }
        Ok(())
    }
}

pub fn ensure_regular(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if let Ok(metadata) = fs::symlink_metadata(parent) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("存储目录必须是普通目录。".into());
            }
        }
    }
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() || !m.is_file() => {
            Err("目标文件不是普通文件，操作已停止。".into())
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("无法访问本地文件，请检查权限。".into()),
    }
}

pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    ensure_regular(path)?;
    match fs::metadata(path) {
        Ok(m) if m.len() > 16 * 1024 * 1024 => return Err("本地文件过大，操作已停止。".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("无法读取本地文件。".into()),
        _ => (),
    }
    fs::read(path)
        .map(Some)
        .map_err(|_| "无法读取本地文件。".into())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure_regular(path)?;
    let parent = path.parent().ok_or("无效的存储路径")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建存储目录")?;
    if fs::symlink_metadata(parent)
        .map_err(|_| "无法访问存储目录")?
        .file_type()
        .is_symlink()
    {
        return Err("存储目录不能是符号链接。".into());
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| "无法创建临时文件")?;
    temp.write_all(bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|_| "写入失败，原文件已保留。")?;
    temp.persist(path)
        .map_err(|_| "无法替换本地文件，原文件已保留。")?;
    #[cfg(unix)]
    {
        let _ = fs::File::open(parent).and_then(|f| f.sync_all());
    }
    Ok(())
}

fn remove_optional(path: &Path) -> Result<()> {
    ensure_regular(path)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("无法清理旧登录状态。".into()),
    }
}
