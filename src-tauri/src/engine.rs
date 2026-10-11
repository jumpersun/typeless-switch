use crate::{
    codec,
    files::{Captured, SessionFiles, Snapshot},
    vault::Vault,
    Result,
};
use serde::{Deserialize, Serialize};

pub trait Client: Send + Sync {
    fn available(&self) -> bool;
    fn running(&self) -> bool;
    fn stop(&self) -> Result<()>;
    fn start(&self) -> Result<()>;
    fn wait_for_account(&self, files: &SessionFiles, id: &str) -> Result<Captured>;
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub session: Captured,
    pub saved_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Pending {
    pub kind: String,
    pub backup: Snapshot,
    pub was_running: bool,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Store {
    pub accounts: Vec<Account>,
    pub pending: Option<Pending>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    pub email: String,
    pub saved_at: u64,
    pub expired: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub accounts: Vec<AccountView>,
    pub current: Option<AccountView>,
    pub pending: Option<String>,
    pub installed: bool,
    pub notice: Option<String>,
}
pub struct Engine {
    pub files: SessionFiles,
    pub vault: Vault,
    pub client: Box<dyn Client>,
}
impl Engine {
    pub fn view(&self) -> Result<View> {
        let _lock = self.vault.lock()?;
        self.view_inner(&self.vault.load()?)
    }
    fn view_inner(&self, store: &Store) -> Result<View> {
        let captured = self.files.capture();
        let notice = captured.as_ref().err().cloned();
        let current = captured.ok().map(|s| summary(&s, 0));
        Ok(View {
            accounts: store
                .accounts
                .iter()
                .map(|a| summary(&a.session, a.saved_at))
                .collect(),
            current,
            pending: store.pending.as_ref().map(|p| p.kind.clone()),
            installed: self.client.available(),
            notice,
        })
    }
    pub fn execute(&self, action: &str, id: Option<&str>) -> Result<View> {
        let _lock = self.vault.lock()?;
        let mut store: Store = self.vault.load()?;
        if store.pending.is_some() && !matches!(action, "restore" | "finish_add") {
            return Err("请先完成登录或恢复上一次操作。".into());
        }
        match action {
            "save" => {
                upsert(&mut store, self.files.capture()?);
                self.vault.save(&store)?;
            }
            "remove" => {
                let id = id.ok_or("请选择账号")?;
                store
                    .accounts
                    .retain(|a| codec::account_id(&a.session.identity.user_id) != id);
                self.vault.save(&store)?;
            }
            "switch" => {
                let target = store
                    .accounts
                    .iter()
                    .find(|a| Some(codec::account_id(&a.session.identity.user_id)).as_deref() == id)
                    .ok_or("找不到这个账号")?
                    .session
                    .clone();
                if expired(&target) {
                    return Err("此账号的保存会话已过期，请通过添加账号重新登录。".into());
                }
                self.begin(&mut store, "switch")?;
                let result = (|| {
                    self.files.apply(Some(&target))?;
                    self.client.start()?;
                    let fresh = self
                        .client
                        .wait_for_account(&self.files, &target.identity.user_id)?;
                    upsert(&mut store, fresh);
                    self.commit(&mut store)
                })();
                if let Err(error) = result {
                    return Err(self.fail_and_restore(&mut store, error));
                }
            }
            "add" => {
                self.begin(&mut store, "add")?;
                if let Err(error) = self.files.apply(None).and_then(|_| self.client.start()) {
                    return Err(self.fail_and_restore(&mut store, error));
                }
            }
            "finish_add" => {
                if store.pending.as_ref().map(|p| p.kind.as_str()) != Some("add") {
                    return Err("没有等待完成的登录。".into());
                }
                let captured = self.files.capture()?;
                if expired(&captured) {
                    return Err("登录状态已过期，请在 Typeless 重新登录。".into());
                }
                upsert(&mut store, captured);
                self.commit(&mut store)?;
            }
            "restore" => self.restore(&mut store)?,
            _ => return Err("不支持的操作".into()),
        }
        self.view_inner(&store)
    }
    fn begin(&self, store: &mut Store, kind: &str) -> Result<()> {
        if !self.client.available() {
            return Err("未找到 Typeless，请先安装官方客户端。".into());
        }
        let was_running = self.client.running();
        self.client.stop()?;
        let preparation = (|| {
            let backup = self.files.snapshot()?;
            // A present but unreadable session may belong to a newer client format.
            // Stop before any mutation instead of silently discarding that identity.
            if backup.session.is_some() {
                upsert(store, self.files.capture()?);
            }
            store.pending = Some(Pending {
                kind: kind.into(),
                backup,
                was_running,
            });
            self.vault.save(store)
        })();
        if let Err(error) = preparation {
            if was_running {
                let _ = self.client.start();
            }
            return Err(error);
        }
        Ok(())
    }
    fn commit(&self, store: &mut Store) -> Result<()> {
        let pending = store.pending.take();
        if let Err(error) = self.vault.save(store) {
            store.pending = pending;
            return Err(error);
        }
        Ok(())
    }
    fn restore(&self, store: &mut Store) -> Result<()> {
        let pending = store.pending.as_ref().ok_or("没有需要恢复的操作")?.clone();
        self.client.stop()?;
        self.files.restore(&pending.backup)?;
        if pending.was_running {
            self.client.start()?;
        }
        self.commit(store)
    }
    fn fail_and_restore(&self, store: &mut Store, error: String) -> String {
        match self.restore(store) {
            Ok(()) => format!("{error} 已恢复切换前的登录状态。"),
            Err(_) => format!("{error} 自动恢复未完成。备份已保留，请点击“恢复原账号”。"),
        }
    }
}
fn expired(s: &Captured) -> bool {
    s.identity
        .refresh_expires_at
        .is_some_and(|e| e <= codec::now())
}
fn summary(s: &Captured, saved_at: u64) -> AccountView {
    AccountView {
        id: codec::account_id(&s.identity.user_id),
        email: s.identity.email.clone(),
        saved_at,
        expired: expired(s),
    }
}
fn upsert(store: &mut Store, session: Captured) {
    if let Some(account) = store
        .accounts
        .iter_mut()
        .find(|a| a.session.identity.user_id == session.identity.user_id)
    {
        account.session = session;
        account.saved_at = codec::now();
    } else {
        store.accounts.push(Account {
            session,
            saved_at: codec::now(),
        });
    }
}
