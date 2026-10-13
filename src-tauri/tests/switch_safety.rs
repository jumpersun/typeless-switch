use std::{
    fs,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use typeless_switch::{
    codec,
    engine::{Client, Engine, Store},
    files::{atomic_write, Captured, SessionFiles},
    vault::{Protector, Vault},
    Result,
};
struct TestProtection;
impl Protector for TestProtection {
    fn seal(&self, v: &[u8]) -> Result<Vec<u8>> {
        Ok([b"test:".as_slice(), v].concat())
    }
    fn open(&self, v: &[u8]) -> Result<Vec<u8>> {
        v.strip_prefix(b"test:")
            .map(|v| v.to_vec())
            .ok_or("corrupt".into())
    }
}
#[derive(Default)]
struct Control {
    starts_fail: AtomicUsize,
    start_timeout_after_launch: AtomicBool,
    running: AtomicBool,
    stop_fail: AtomicBool,
    stop_attempts: AtomicUsize,
    stop_fail_at: AtomicUsize,
    confirm_fail: AtomicBool,
}
struct FakeClient {
    root: std::path::PathBuf,
    control: Arc<Control>,
}
impl Client for FakeClient {
    fn available(&self) -> bool {
        true
    }
    fn running(&self) -> bool {
        self.control.running.load(Ordering::SeqCst)
    }
    fn stop(&self) -> Result<()> {
        let attempt = self.control.stop_attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if self.control.stop_fail.load(Ordering::SeqCst)
            || self.control.stop_fail_at.load(Ordering::SeqCst) == attempt
        {
            return Err("quit request timed out".into());
        }
        self.control.running.store(false, Ordering::SeqCst);
        Ok(())
    }
    fn start(&self) -> Result<()> {
        if self
            .control
            .starts_fail
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1))
            .is_ok()
        {
            return Err("start failed".into());
        }
        self.control.running.store(true, Ordering::SeqCst);
        if let Ok(bytes) = fs::read(self.root.join("user-data.json")) {
            let (id, _) = codec::inspect(&bytes, &["darwin-arm64/Typeless".into()])?;
            let mut cache: serde_json::Value =
                serde_json::from_slice(&fs::read(self.root.join("app-storage.json")).unwrap())
                    .unwrap();
            cache["userData"] = serde_json::json!({"user_id":id.user_id,"email":id.email});
            atomic_write(
                &self.root.join("app-storage.json"),
                &serde_json::to_vec(&cache).unwrap(),
            )?;
        }
        // A launch helper can time out after the official client already started.
        if self
            .control
            .start_timeout_after_launch
            .swap(false, Ordering::SeqCst)
        {
            return Err("launch request timed out".into());
        }
        Ok(())
    }
    fn wait_for_account(&self, files: &SessionFiles, id: &str) -> Result<Captured> {
        if self.control.confirm_fail.load(Ordering::SeqCst) {
            return Err("account confirmation timed out".into());
        }
        let captured = files.capture()?;
        if captured.identity.user_id != id {
            return Err("wrong identity".into());
        }
        Ok(captured)
    }
}
fn fixture(name: &str) -> Vec<u8> {
    fs::read(format!(
        "{}/tests/fixtures/darwin-arm64-{name}.bin",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
struct Rig {
    _temp: tempfile::TempDir,
    engine: Engine,
    control: Arc<Control>,
}
impl Rig {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Typeless");
        fs::create_dir_all(&root).unwrap();
        let control = Arc::new(Control::default());
        control.running.store(true, Ordering::SeqCst);
        let engine = Engine {
            files: SessionFiles {
                root: root.clone(),
                profiles: vec!["darwin-arm64/Typeless".into()],
            },
            vault: Vault {
                root: temp.path().join("vault"),
                protector: Box::new(TestProtection),
            },
            client: Box::new(FakeClient {
                root,
                control: control.clone(),
            }),
        };
        let rig = Self {
            _temp: temp,
            engine,
            control,
        };
        rig.login("alice");
        for name in [
            "typeless.db",
            "audio.wav",
            "app-settings.json",
            "device.cache",
        ] {
            fs::write(rig.engine.files.root.join(name), b"preserve-exactly").unwrap();
        }
        rig
    }
    fn login(&self, name: &str) {
        fs::write(self.engine.files.root.join("user-data.json"), fixture(name)).unwrap();
        fs::write(self.engine.files.root.join("app-storage.json"), serde_json::to_vec(&serde_json::json!({"userData":{"user_id":name,"email":format!("{name}@example.test")},"accessibilityConfig":{"enabled":true},"__internal__":{"version":1},"TYPELESS_TIME_DIFF":0})).unwrap()).unwrap();
    }
    fn two_accounts(&self) {
        self.engine.execute("save", None).unwrap();
        self.login("bob");
        self.engine.execute("save", None).unwrap();
        self.login("alice");
    }
    fn switch_bob(&self) -> Result<typeless_switch::engine::View> {
        self.engine
            .execute("switch", Some(&codec::account_id("bob")))
    }
    fn check_alice(&self) {
        assert_eq!(
            self.engine.files.capture().unwrap().identity.user_id,
            "alice"
        );
    }
}
#[test]
fn node_sessions_match_rust_on_both_platforms_and_invalid_utf8_iv() {
    for (platform, profile) in [
        ("darwin-arm64", "darwin-arm64/Typeless"),
        ("darwin-x64", "darwin-x64/Typeless"),
        ("win32-x64", "win32-x64/Typeless.exe"),
    ] {
        let bytes = fs::read(format!(
            "{}/tests/fixtures/{platform}-alice.bin",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let (id, _) = codec::inspect(&bytes, &[profile.into()]).unwrap();
        assert_eq!(id.email, "alice@example.test");
        assert_eq!(id.refresh_expires_at, Some(4102444800));
        assert!(codec::inspect(&bytes, &["wrong/Typeless".into()]).is_err());
    }
}
#[test]
fn switches_both_directions_and_preserves_settings_history_audio() {
    let rig = Rig::new();
    rig.two_accounts();
    let view = rig.switch_bob().unwrap();
    assert_eq!(view.current.unwrap().email, "bob@example.test");
    assert!(view.pending.is_none());
    rig.engine
        .execute("switch", Some(&codec::account_id("alice")))
        .unwrap();
    rig.check_alice();
    for name in [
        "typeless.db",
        "audio.wav",
        "app-settings.json",
        "device.cache",
    ] {
        assert_eq!(
            fs::read(rig.engine.files.root.join(name)).unwrap(),
            b"preserve-exactly"
        );
    }
    let cache: serde_json::Value =
        serde_json::from_slice(&fs::read(rig.engine.files.root.join("app-storage.json")).unwrap())
            .unwrap();
    assert_eq!(cache["accessibilityConfig"]["enabled"], true);
    assert_eq!(cache["__internal__"]["version"], 1);
}
#[test]
fn startup_failure_rolls_back() {
    let rig = Rig::new();
    rig.two_accounts();
    rig.control.starts_fail.store(1, Ordering::SeqCst);
    assert!(rig.switch_bob().unwrap_err_string().contains("已恢复"));
    rig.check_alice();
    assert!(rig.engine.view().unwrap().pending.is_none());
}
#[test]
fn launch_timeout_stops_started_target_before_restoring_original() {
    let rig = Rig::new();
    rig.two_accounts();
    let before = rig.engine.files.snapshot().unwrap();
    rig.control
        .start_timeout_after_launch
        .store(true, Ordering::SeqCst);

    let error = rig.switch_bob().unwrap_err_string();

    assert!(error.contains("launch request timed out"));
    assert!(error.contains("已恢复"));
    assert_eq!(rig.control.stop_attempts.load(Ordering::SeqCst), 2);
    assert!(rig.control.running.load(Ordering::SeqCst));
    let after = rig.engine.files.snapshot().unwrap();
    assert_eq!(after.session, before.session);
    assert_eq!(after.storage, before.storage);
    let persisted: Store = rig.engine.vault.load().unwrap();
    assert!(persisted.pending.is_none());
    rig.check_alice();
}
trait ErrorText {
    fn unwrap_err_string(self) -> String;
}
impl ErrorText for Result<typeless_switch::engine::View> {
    fn unwrap_err_string(self) -> String {
        match self {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        }
    }
}
#[test]
fn failed_confirmation_rolls_back() {
    let rig = Rig::new();
    rig.two_accounts();
    let before = rig.engine.files.snapshot().unwrap();
    rig.control.confirm_fail.store(true, Ordering::SeqCst);
    let error = rig.switch_bob().unwrap_err_string();
    assert!(error.contains("account confirmation timed out"));
    assert!(error.contains("已恢复"));
    let after = rig.engine.files.snapshot().unwrap();
    assert_eq!(after.session, before.session);
    assert_eq!(after.storage, before.storage);
    let persisted: Store = rig.engine.vault.load().unwrap();
    assert!(persisted.pending.is_none());
    assert_eq!(persisted.accounts.len(), 2);
    rig.check_alice();
}
#[test]
fn rollback_quit_timeout_retains_journal_until_explicit_recovery() {
    let rig = Rig::new();
    rig.two_accounts();
    let original = rig.engine.files.snapshot().unwrap();
    rig.control.confirm_fail.store(true, Ordering::SeqCst);
    rig.control.stop_fail_at.store(2, Ordering::SeqCst);

    let error = rig.switch_bob().unwrap_err_string();

    assert!(error.contains("备份已保留"));
    assert!(!error.contains("已恢复切换前"));
    assert!(rig.control.running.load(Ordering::SeqCst));
    assert_eq!(rig.engine.files.capture().unwrap().identity.user_id, "bob");
    let persisted: Store = rig.engine.vault.load().unwrap();
    let pending = persisted.pending.unwrap();
    assert_eq!(pending.kind, "switch");
    assert_eq!(pending.backup.session, original.session);
    assert_eq!(pending.backup.storage, original.storage);
    let failed_state = rig.engine.files.snapshot().unwrap();
    for (action, id) in [
        ("save", None),
        ("add", None),
        ("switch", Some(codec::account_id("alice"))),
        ("remove", Some(codec::account_id("alice"))),
    ] {
        assert!(rig.engine.execute(action, id.as_deref()).is_err());
    }
    let unchanged = rig.engine.files.snapshot().unwrap();
    assert_eq!(unchanged.session, failed_state.session);
    assert_eq!(unchanged.storage, failed_state.storage);

    rig.engine.execute("restore", None).unwrap();

    rig.check_alice();
    assert!(rig.control.running.load(Ordering::SeqCst));
    let after = rig.engine.files.snapshot().unwrap();
    assert_eq!(after.session, original.session);
    assert_eq!(after.storage, original.storage);
    let persisted: Store = rig.engine.vault.load().unwrap();
    assert!(persisted.pending.is_none());
}
#[test]
fn confirmation_timeout_restores_without_starting_previously_closed_client() {
    let rig = Rig::new();
    rig.two_accounts();
    let before = rig.engine.files.snapshot().unwrap();
    rig.control.running.store(false, Ordering::SeqCst);
    rig.control.confirm_fail.store(true, Ordering::SeqCst);

    assert!(rig.switch_bob().unwrap_err_string().contains("已恢复"));

    assert!(!rig.control.running.load(Ordering::SeqCst));
    let after = rig.engine.files.snapshot().unwrap();
    assert_eq!(after.session, before.session);
    assert_eq!(after.storage, before.storage);
    assert!(rig.engine.view().unwrap().pending.is_none());
}
#[test]
fn failed_rollback_keeps_journal_and_blocks_mutations() {
    let rig = Rig::new();
    rig.two_accounts();
    rig.control.starts_fail.store(2, Ordering::SeqCst);
    assert!(rig.switch_bob().unwrap_err_string().contains("备份已保留"));
    assert_eq!(
        rig.engine.view().unwrap().pending.as_deref(),
        Some("switch")
    );
    assert!(rig
        .engine
        .execute("remove", Some(&codec::account_id("alice")))
        .is_err());
    rig.engine.execute("restore", None).unwrap();
    rig.check_alice();
}
#[test]
fn interrupted_add_can_be_restored_from_disk() {
    let rig = Rig::new();
    let original = rig.engine.files.snapshot().unwrap();
    rig.engine.execute("add", None).unwrap();
    let persisted: Store = rig.engine.vault.load().unwrap();
    assert_eq!(persisted.pending.unwrap().kind, "add");
    assert!(rig.engine.execute("finish_add", None).is_err());
    rig.engine.execute("restore", None).unwrap();
    rig.check_alice();
    assert_eq!(
        rig.engine.files.snapshot().unwrap().session,
        original.session
    );
}
#[test]
fn add_uses_official_login_then_deduplicates() {
    let rig = Rig::new();
    rig.engine.execute("add", None).unwrap();
    rig.login("bob");
    let view = rig.engine.execute("finish_add", None).unwrap();
    assert_eq!(view.accounts.len(), 2);
    assert!(view.pending.is_none());
    assert_eq!(rig.engine.execute("save", None).unwrap().accounts.len(), 2);
}
#[test]
fn removing_current_saved_account_keeps_client_login_and_can_be_saved_again() {
    let rig = Rig::new();
    rig.engine.execute("save", None).unwrap();
    let before = rig.engine.files.snapshot().unwrap();
    let id = codec::account_id("alice");

    let view = rig.engine.execute("remove", Some(&id)).unwrap();

    assert!(view.accounts.is_empty());
    assert_eq!(view.current.unwrap().id, id);
    assert!(view.pending.is_none());
    assert!(rig.control.running.load(Ordering::SeqCst));
    assert_eq!(rig.control.stop_attempts.load(Ordering::SeqCst), 0);
    let after = rig.engine.files.snapshot().unwrap();
    assert_eq!(after.session, before.session);
    assert_eq!(after.storage, before.storage);
    let persisted: Store = rig.engine.vault.load().unwrap();
    assert!(persisted.accounts.is_empty());

    let saved = rig.engine.execute("save", None).unwrap();

    assert_eq!(saved.accounts.len(), 1);
    assert_eq!(saved.accounts[0].id, id);
    rig.check_alice();
}
#[test]
fn expired_or_unreadable_session_never_replaces_current() {
    let rig = Rig::new();
    rig.login("expired");
    rig.engine.execute("save", None).unwrap();
    rig.login("alice");
    assert!(rig
        .engine
        .execute("switch", Some(&codec::account_id("expired")))
        .is_err());
    rig.check_alice();
    assert!(codec::inspect(b"invalid", &rig.engine.files.profiles).is_err());
}
#[test]
fn locked_vault_and_failed_quit_do_not_modify_session() {
    let rig = Rig::new();
    rig.two_accounts();
    let lock = rig.engine.vault.lock().unwrap();
    assert!(rig.switch_bob().is_err());
    drop(lock);
    rig.check_alice();
    rig.control.stop_fail.store(true, Ordering::SeqCst);
    assert!(rig.switch_bob().is_err());
    rig.check_alice();
}
#[test]
fn corrupt_vault_fails_closed() {
    let rig = Rig::new();
    rig.two_accounts();
    fs::write(rig.engine.vault.root.join("vault.bin"), b"corrupted").unwrap();
    assert!(rig.switch_bob().is_err());
    rig.check_alice();
    assert_eq!(
        fs::read(rig.engine.vault.root.join("vault.bin")).unwrap(),
        b"corrupted"
    );
}
#[cfg(unix)]
#[test]
fn symlink_session_is_rejected_without_touching_target() {
    let rig = Rig::new();
    let path = rig.engine.files.root.join("user-data.json");
    fs::remove_file(&path).unwrap();
    let target = rig.engine.files.root.join("unrelated");
    fs::write(&target, b"unchanged").unwrap();
    std::os::unix::fs::symlink(&target, path).unwrap();
    assert!(rig.engine.execute("add", None).is_err());
    assert_eq!(fs::read(target).unwrap(), b"unchanged");
}
struct FailSeal {
    attempt: AtomicUsize,
    fail_at: usize,
}
impl Protector for FailSeal {
    fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        if self.attempt.fetch_add(1, Ordering::SeqCst) + 1 == self.fail_at {
            return Err("vault write failure".into());
        }
        TestProtection.seal(bytes)
    }
    fn open(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        TestProtection.open(bytes)
    }
}
#[test]
fn journal_write_failure_prevents_any_auth_change() {
    let mut rig = Rig::new();
    rig.two_accounts();
    let before = rig.engine.files.snapshot().unwrap();
    rig.engine.vault.protector = Box::new(FailSeal {
        attempt: AtomicUsize::new(0),
        fail_at: 1,
    });
    assert!(rig.switch_bob().is_err());
    let after = rig.engine.files.snapshot().unwrap();
    assert_eq!(before.session, after.session);
    assert_eq!(before.storage, after.storage);
    assert!(rig.engine.view().unwrap().pending.is_none());
}
#[test]
fn failed_final_commit_restores_original_and_keeps_accounts() {
    let mut rig = Rig::new();
    rig.two_accounts();
    rig.engine.vault.protector = Box::new(FailSeal {
        attempt: AtomicUsize::new(0),
        fail_at: 2,
    });
    assert!(rig.switch_bob().unwrap_err_string().contains("已恢复"));
    rig.check_alice();
    let view = rig.engine.view().unwrap();
    assert!(view.pending.is_none());
    assert_eq!(view.accounts.len(), 2);
}
#[test]
fn mismatched_cache_cannot_be_saved_as_a_valid_account() {
    let rig = Rig::new();
    fs::write(rig.engine.files.root.join("user-data.json"), fixture("bob")).unwrap();
    assert!(rig.engine.execute("save", None).is_err());
    assert!(rig.engine.view().unwrap().accounts.is_empty());
}
#[test]
fn unknown_current_format_blocks_switch_and_add_without_mutation() {
    let rig = Rig::new();
    rig.two_accounts();
    fs::write(
        rig.engine.files.root.join("user-data.json"),
        b"future-client-format",
    )
    .unwrap();
    let before = rig.engine.files.snapshot().unwrap();
    assert!(rig.switch_bob().is_err());
    assert!(rig.engine.execute("add", None).is_err());
    let after = rig.engine.files.snapshot().unwrap();
    assert_eq!(before.session, after.session);
    assert_eq!(before.storage, after.storage);
    assert!(rig.engine.view().unwrap().pending.is_none());
}

// Old optional-feature files must never participate in account transactions.
fn write_legacy_toolkit(rig: &Rig) -> Vec<u8> {
    fs::create_dir_all(&rig.engine.vault.root).unwrap();
    let data = serde_json::json!({"autoSync": true, "apiAutoSync": true});
    let bytes = TestProtection.seal(&serde_json::to_vec(&data).unwrap()).unwrap();
    fs::write(rig.engine.vault.root.join("toolkit.bin"), &bytes).unwrap();
    bytes
}

#[test]
fn legacy_sync_settings_do_not_block_switching_in_either_direction() {
    let rig = Rig::new();
    rig.two_accounts();
    let legacy = write_legacy_toolkit(&rig);
    let view = rig.switch_bob().unwrap();
    assert_eq!(view.current.unwrap().id, codec::account_id("bob"));
    assert!(view.pending.is_none());
    assert!(view.notice.is_none());
    rig.engine.execute("switch", Some(&codec::account_id("alice"))).unwrap();
    rig.check_alice();
    assert_eq!(fs::read(rig.engine.vault.root.join("toolkit.bin")).unwrap(), legacy);
}

#[test]
fn legacy_sync_settings_do_not_block_adding_and_saving_an_account() {
    let rig = Rig::new();
    let legacy = write_legacy_toolkit(&rig);
    rig.engine.execute("add", None).unwrap();
    rig.login("bob");
    let view = rig.engine.execute("finish_add", None).unwrap();
    assert!(view.pending.is_none());
    assert_eq!(view.current.unwrap().id, codec::account_id("bob"));
    assert!(view.notice.is_none());
    assert_eq!(view.accounts.len(), 2);
    assert_eq!(fs::read(rig.engine.vault.root.join("toolkit.bin")).unwrap(), legacy);
}

#[test]
fn corrupt_legacy_toolkit_does_not_block_switch_or_recovery() {
    let rig = Rig::new();
    rig.two_accounts();
    let path = rig.engine.vault.root.join("toolkit.bin");
    fs::write(&path, b"unreadable legacy data").unwrap();
    rig.switch_bob().unwrap();
    assert_eq!(rig.engine.files.capture().unwrap().identity.user_id, "bob");
    rig.engine.execute("add", None).unwrap();
    rig.engine.execute("restore", None).unwrap();
    assert_eq!(rig.engine.files.capture().unwrap().identity.user_id, "bob");
    assert_eq!(fs::read(path).unwrap(), b"unreadable legacy data");
}
