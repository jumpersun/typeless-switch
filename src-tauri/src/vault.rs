use crate::{
    files::{atomic_write, ensure_regular, read_optional},
    Result,
};
use fs2::FileExt;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
};
use zeroize::Zeroizing;

pub trait Protector: Send + Sync {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>>;
    fn open(&self, encrypted: &[u8]) -> Result<Vec<u8>>;
}
pub struct SystemProtector;

pub struct Vault {
    pub root: PathBuf,
    pub protector: Box<dyn Protector>,
}
impl Vault {
    pub fn lock(&self) -> Result<File> {
        fs::create_dir_all(&self.root).map_err(|_| "无法创建账号库")?;
        if fs::symlink_metadata(&self.root)
            .map_err(|_| "无法访问账号库")?
            .file_type()
            .is_symlink()
        {
            return Err("账号库不能使用符号链接".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法设置账号库权限")?;
        }
        let path = self.root.join("operation.lock");
        ensure_regular(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|_| "无法锁定账号库")?;
        file.try_lock_exclusive()
            .map_err(|_| "另一个窗口正在操作账号，请稍后重试。")?;
        Ok(file)
    }
    pub fn load<T: DeserializeOwned + Default>(&self) -> Result<T> {
        self.load_file("vault.bin")
    }
    pub(crate) fn load_file<T: DeserializeOwned + Default>(&self, name: &str) -> Result<T> {
        let Some(bytes) = read_optional(&self.root.join(name))? else {
            return Ok(T::default());
        };
        let plain = Zeroizing::new(self.protector.open(&bytes)?);
        serde_json::from_slice(&plain).map_err(|_| "账号库格式损坏，原文件已保留。".into())
    }
    pub fn save<T: Serialize>(&self, value: &T) -> Result<()> {
        self.save_file("vault.bin", value)
    }
    pub(crate) fn save_file<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let plain = Zeroizing::new(serde_json::to_vec(value).map_err(|_| "无法保存账号库")?);
        let encrypted = self.protector.seal(&plain)?;
        if encrypted.len() > 16 * 1024 * 1024 {
            return Err("账号库超过大小限制，请移除不再使用的账号后重试。".into());
        }
        atomic_write(&self.root.join(name), &encrypted)
    }
}

#[cfg(target_os = "macos")]
mod system {
    use super::*;
    use aes_gcm::{
        aead::{Aead, KeyInit, Payload},
        Aes256Gcm, Nonce,
    };
    use rand::RngCore;
    const SERVICE: &str = "io.typeless-switch.app";
    const AAD: &[u8] = b"typeless-switch-vault-v1";
    fn key(create: bool) -> Result<Zeroizing<Vec<u8>>> {
        use security_framework::passwords::{get_generic_password, set_generic_password};
        match get_generic_password(SERVICE, "vault-key-v1") {
            Ok(key) if key.len() == 32 => Ok(Zeroizing::new(key)),
            Err(e) if e.code() == -25300 && create => {
                let mut key = Zeroizing::new(vec![0u8; 32]);
                rand::rngs::OsRng.fill_bytes(&mut key);
                set_generic_password(SERVICE, "vault-key-v1", &key)
                    .map_err(|_| "无法保存钥匙串密钥。")?;
                Ok(key)
            }
            _ => Err("无法访问账号库的钥匙串密钥，请解锁登录钥匙串并重试。".into()),
        }
    }
    impl Protector for SystemProtector {
        fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
            let key = key(true)?;
            let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "账号库密钥无效")?;
            let mut nonce = [0u8; 12];
            rand::rngs::OsRng.fill_bytes(&mut nonce);
            let encrypted = cipher
                .encrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: plain,
                        aad: AAD,
                    },
                )
                .map_err(|_| "无法加密账号库")?;
            Ok([b"TSW1".as_slice(), &nonce, &encrypted].concat())
        }
        fn open(&self, data: &[u8]) -> Result<Vec<u8>> {
            if data.len() < 32 || &data[..4] != b"TSW1" {
                return Err("账号库格式不受支持。".into());
            }
            let key = key(false)?;
            Aes256Gcm::new_from_slice(&key)
                .map_err(|_| "账号库密钥无效")?
                .decrypt(
                    Nonce::from_slice(&data[4..16]),
                    Payload {
                        msg: &data[16..],
                        aad: AAD,
                    },
                )
                .map_err(|_| "账号库解密失败，原文件已保留。".into())
        }
    }
}

#[cfg(target_os = "windows")]
mod system {
    use super::*;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    fn transform(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len().try_into().map_err(|_| "账号库过大")?,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // DPAPI owns the output allocation; free it after copying, on the same Windows user.
        let ok = unsafe {
            if encrypt {
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            } else {
                CryptUnprotectData(
                    &input,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            }
        };
        if ok == 0 {
            return Err("Windows 无法保护或读取账号库，请使用保存它的系统用户。".into());
        }
        let result =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe {
            std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
            LocalFree(output.pbData.cast());
        }
        Ok(result)
    }
    impl Protector for SystemProtector {
        fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
            transform(plain, true)
        }
        fn open(&self, data: &[u8]) -> Result<Vec<u8>> {
            transform(data, false)
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl Protector for SystemProtector {
    fn seal(&self, _: &[u8]) -> Result<Vec<u8>> {
        Err("此版本支持 macOS 和 Windows。".into())
    }
    fn open(&self, _: &[u8]) -> Result<Vec<u8>> {
        Err("此版本支持 macOS 和 Windows。".into())
    }
}
