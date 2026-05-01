//! Tests for ghcp_mon::local_session. LLRs:
//! - Local session state dir resolved from flag env or home
//! - Local session workspace yaml best effort read
//! - Local session workspace yaml rejects path traversal

use ghcp_mon::local_session::{
    default_session_state_dir, read_workspace_yaml, resolve_session_state_dir,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// `std::env::set_var` is process-global; serialize env-mutating tests.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn unique_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-ls-{}-{}-{}", tag, std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}
impl EnvGuard {
    fn new(keys: &[&'static str]) -> Self {
        let saved = keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();
        Self { saved }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in self.saved.drain(..) {
            match v {
                Some(s) => std::env::set_var(k, s),
                None => std::env::remove_var(k),
            }
        }
    }
}

#[test]
fn resolve_flag_override_takes_precedence() {
    let _g = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::new(&["COPILOT_SESSION_STATE_DIR", "HOME"]);
    std::env::set_var("COPILOT_SESSION_STATE_DIR", "/from/env");
    std::env::set_var("HOME", "/home/x");
    let flag = PathBuf::from("/from/flag");
    let resolved = resolve_session_state_dir(Some(&flag));
    assert_eq!(resolved, Some(PathBuf::from("/from/flag")));
}

#[test]
fn resolve_uses_env_var_when_no_flag() {
    let _g = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::new(&["COPILOT_SESSION_STATE_DIR", "HOME"]);
    std::env::set_var("COPILOT_SESSION_STATE_DIR", "/env/dir");
    std::env::set_var("HOME", "/home/x");
    let resolved = resolve_session_state_dir(None);
    assert_eq!(resolved, Some(PathBuf::from("/env/dir")));
}

#[test]
fn resolve_falls_back_to_home_when_no_flag_no_env() {
    let _g = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::new(&["COPILOT_SESSION_STATE_DIR", "HOME"]);
    std::env::remove_var("COPILOT_SESSION_STATE_DIR");
    std::env::set_var("HOME", "/home/foo");
    let resolved = resolve_session_state_dir(None);
    assert_eq!(resolved, Some(PathBuf::from("/home/foo/.copilot/session-state")));
}

#[test]
fn resolve_returns_none_when_nothing_set() {
    let _g = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::new(&["COPILOT_SESSION_STATE_DIR", "HOME"]);
    std::env::remove_var("COPILOT_SESSION_STATE_DIR");
    std::env::remove_var("HOME");
    assert_eq!(resolve_session_state_dir(None), None);
}

#[test]
fn resolve_treats_empty_env_var_as_unset() {
    let _g = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::new(&["COPILOT_SESSION_STATE_DIR", "HOME"]);
    std::env::set_var("COPILOT_SESSION_STATE_DIR", "");
    std::env::set_var("HOME", "/home/x");
    let resolved = resolve_session_state_dir(None);
    assert_eq!(resolved, Some(PathBuf::from("/home/x/.copilot/session-state")));
}

#[test]
fn default_session_state_dir_equivalent_to_resolve_none() {
    let _g = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::new(&["COPILOT_SESSION_STATE_DIR", "HOME"]);
    std::env::set_var("COPILOT_SESSION_STATE_DIR", "/some/dir");
    assert_eq!(default_session_state_dir(), resolve_session_state_dir(None));
}

#[test]
fn read_workspace_yaml_parses_known_fields() {
    let base = unique_dir("yaml-ok");
    let cid = "abc123";
    std::fs::create_dir_all(base.join(cid)).unwrap();
    let yaml = "id: abc123\nname: My Project\nuser_named: true\nbranch: main\ncwd: /tmp/x\n";
    std::fs::write(base.join(cid).join("workspace.yaml"), yaml).unwrap();
    let got = read_workspace_yaml(&base, cid).expect("MUST read");
    assert_eq!(got.id.as_deref(), Some("abc123"));
    assert_eq!(got.name.as_deref(), Some("My Project"));
    assert_eq!(got.user_named, Some(true));
    assert_eq!(got.branch.as_deref(), Some("main"));
    assert_eq!(got.cwd.as_deref(), Some("/tmp/x"));
}

#[test]
fn read_workspace_yaml_returns_none_on_missing_file() {
    let base = unique_dir("yaml-missing");
    assert!(read_workspace_yaml(&base, "nonexistent").is_none());
}

#[test]
fn read_workspace_yaml_returns_none_on_bad_yaml() {
    let base = unique_dir("yaml-bad");
    let cid = "x";
    std::fs::create_dir_all(base.join(cid)).unwrap();
    std::fs::write(base.join(cid).join("workspace.yaml"), "::: not yaml :::\n[oops").unwrap();
    assert!(read_workspace_yaml(&base, cid).is_none());
}

#[test]
fn read_workspace_yaml_rejects_traversal_without_touching_fs() {
    // No files exist for these cids — function MUST short-circuit.
    let base = Path::new("/this/path/does/not/exist/aaaaa");
    for bad in ["", "..", "../etc", "a/b", "a\\b", "x/..", "..\\y"] {
        assert!(read_workspace_yaml(base, bad).is_none(), "MUST reject {:?}", bad);
    }
}
