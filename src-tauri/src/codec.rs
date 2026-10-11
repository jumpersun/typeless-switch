//! Read the existing desktop session without minting tokens or changing device identity.
use crate::Result;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub user_id: String,
    pub email: String,
    pub refresh_expires_at: Option<u64>,
}

pub fn profiles() -> Vec<String> {
    let platforms: &[&str] = if cfg!(target_os = "macos") {
        &["darwin-arm64", "darwin-x64"]
    } else {
        &["win32-x64", "win32-arm64", "win32-ia32"]
    };
    platforms
        .iter()
        .flat_map(|p| ["Typeless", "Typeless.exe", "typeless"].map(|n| format!("{p}/{n}")))
        .collect()
}

pub fn inspect(bytes: &[u8], candidates: &[String]) -> Result<(Identity, String)> {
    if bytes.len() < 33
        || bytes.len() > 1024 * 1024
        || bytes[16] != b':'
        || (bytes.len() - 17) % 16 != 0
    {
        return Err("登录文件格式不受支持。请在 Typeless 中重新登录。".into());
    }
    for profile in candidates {
        if let Ok(identity) = decode(bytes, profile) {
            return Ok((identity, profile.clone()));
        }
    }
    Err("无法读取此版本的登录状态，未修改任何文件。".into())
}

fn decode_user<T>(bytes: &[u8], profile: &str, extract: impl FnOnce(&Value) -> Result<T>) -> Result<T> {
    if bytes.len() < 33 || bytes.len() > 1024 * 1024 || bytes[16] != b':' || (bytes.len() - 17) % 16 != 0 { return Err("会话不可读".into()); }
    let (platform, name) = profile.split_once('/').ok_or("未知会话格式")?;
    let seed = format!("{:x}{name}", Sha256::digest(platform.as_bytes()));
    let mut app_key = Zeroizing::new([0u8; 32]);
    pbkdf2_hmac::<Sha256>(
        seed.as_bytes(),
        b"typeless-user-service",
        10_000,
        &mut *app_key,
    );
    let mut key = Zeroizing::new([0u8; 32]);
    // electron-store uses Buffer.toString(), including UTF-8 replacement for random IV bytes.
    pbkdf2_hmac::<Sha512>(
        &*app_key,
        String::from_utf8_lossy(&bytes[..16]).as_bytes(),
        10_000,
        &mut *key,
    );
    let mut buffer = Zeroizing::new(bytes[17..].to_vec());
    let plain = cbc::Decryptor::<aes::Aes256>::new_from_slices(&*key, &bytes[..16])
        .map_err(|_| "会话不可读")?
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|_| "会话不可读")?;
    let mut outer: Value = serde_json::from_slice(plain).map_err(|_| "会话不可读")?;
    let parsed = match outer.get("userData") {
        Some(Value::String(raw)) => serde_json::from_str::<Value>(raw).map_err(|_| "会话不可读"),
        Some(value) => Ok(value.clone()),
        None => Err("会话不可读"),
    };
    wipe(&mut outer);
    let mut user = parsed?;
    let result = extract(&user);
    wipe(&mut user);
    result
}

fn decode(bytes: &[u8], profile: &str) -> Result<Identity> {
    decode_user(bytes, profile, identity_from)
}
fn identity_from(user: &Value) -> Result<Identity> {
    let text = |key: &str| {
        user.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    };
    let user_id = text("user_id").ok_or("缺少账号身份")?;
    let email = text("email")
        .filter(|s| s.contains('@') && s.len() <= 320)
        .ok_or("缺少账号邮箱")?;
    text("access_token").ok_or("缺少登录状态")?;
    let refresh = text("refresh_token").ok_or("缺少续期状态")?;
    let expiry = refresh
        .split('.')
        .nth(1)
        .and_then(|p| URL_SAFE_NO_PAD.decode(p).ok())
        .and_then(|p| serde_json::from_slice::<Value>(&p).ok())
        .and_then(|p| p.get("exp").and_then(Value::as_u64));
    Ok(Identity {
        user_id: user_id.into(),
        email: email.into(),
        refresh_expires_at: expiry,
    })
}

fn wipe(value: &mut Value) {
    match value {
        Value::String(s) => s.zeroize(),
        Value::Array(a) => a.iter_mut().for_each(wipe),
        Value::Object(m) => m.values_mut().for_each(wipe),
        _ => (),
    }
}

pub fn account_id(user_id: &str) -> String {
    format!("{:x}", Sha256::digest(user_id.as_bytes()))
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
