//! Reads sidecar metadata that the GitHub Copilot CLI writes to
//! `~/.copilot/session-state/<conversation_id>/workspace.yaml`.
//!
//! That file is the only place where a session's human-readable name
//! (set via `/rename` or auto-summarized) lives — OTel never carries
//! it. Reads are best-effort: a missing dir, missing file, or parse
//! failure all degrade silently to `None`.
//!
//! Resolution rules for the base directory (in priority order):
//!   1. `--session-state-dir` CLI flag / config (passed in explicitly).
//!   2. `$COPILOT_SESSION_STATE_DIR` env var.
//!   3. `$HOME/.copilot/session-state`.
//!
//! We never write here.
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceYaml {
    pub id: Option<String>,
    pub name: Option<String>,
    pub user_named: Option<bool>,
    pub summary: Option<String>,
    pub cwd: Option<String>,
    pub git_root: Option<String>,
    pub branch: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Resolve the session-state directory.
///
/// Precedence:
///   1. `--session-state-dir` CLI flag / config (passed in explicitly).
///   2. `$COPILOT_SESSION_STATE_DIR` env var.
///   3. `$HOME/.copilot/session-state`.
///
/// Returns `None` if no override is supplied and `$HOME` is missing —
/// callers treat that as "no local metadata available".
pub fn resolve_session_state_dir(override_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = override_dir {
        return Some(p.to_path_buf());
    }
    if let Ok(v) = std::env::var("COPILOT_SESSION_STATE_DIR") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let home = std::env::var("HOME").ok()?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".copilot").join("session-state"))
}

/// Backwards-compatible shorthand for `resolve_session_state_dir(None)`.
pub fn default_session_state_dir() -> Option<PathBuf> {
    resolve_session_state_dir(None)
}

/// Read `<base>/<cid>/workspace.yaml`. Best-effort: any error → None.
pub fn read_workspace_yaml(base: &Path, cid: &str) -> Option<WorkspaceYaml> {
    if cid.is_empty() {
        return None;
    }
    // Defense-in-depth: reject anything that looks like a path
    // component so we never escape the base dir.
    if cid.contains('/') || cid.contains('\\') || cid.contains("..") {
        return None;
    }
    let path = base.join(cid).join("workspace.yaml");
    let bytes = std::fs::read(&path).ok()?;
    serde_yaml_ng::from_slice::<WorkspaceYaml>(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_real_shape() {
        let sample = br#"
id: 19c381fa-2501-4c77-b7fe-d9f4a599eaf4
cwd: /home/mabeltma/git/ghcp-mon
git_root: /home/mabeltma/git/ghcp-mon
branch: master
name: Show Last Two Git Commits
user_named: false
summary: Show Last Two Git Commits
summary_count: 0
created_at: 2026-04-28T19:47:21.490Z
updated_at: 2026-04-28T19:47:31.034Z
"#;
        let parsed: WorkspaceYaml = serde_yaml_ng::from_slice(sample).unwrap();
        assert_eq!(parsed.name.as_deref(), Some("Show Last Two Git Commits"));
        assert_eq!(parsed.user_named, Some(false));
        assert_eq!(parsed.branch.as_deref(), Some("master"));
    }

    #[test]
    fn missing_dir_returns_none() {
        let dir = std::env::temp_dir().join("ghcp-mon-test-missing");
        let r = read_workspace_yaml(&dir, "does-not-exist");
        assert!(r.is_none());
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = std::env::temp_dir();
        assert!(read_workspace_yaml(&dir, "../etc").is_none());
        assert!(read_workspace_yaml(&dir, "a/b").is_none());
        assert!(read_workspace_yaml(&dir, "").is_none());
    }

    #[test]
    fn reads_real_file() {
        let tmp = tempdir_unique();
        let cid = "00000000-0000-0000-0000-000000000001";
        let dir = tmp.join(cid);
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(dir.join("workspace.yaml")).unwrap();
        writeln!(f, "id: {}", cid).unwrap();
        writeln!(f, "name: Hello World").unwrap();
        writeln!(f, "user_named: true").unwrap();
        let r = read_workspace_yaml(&tmp, cid).unwrap();
        assert_eq!(r.name.as_deref(), Some("Hello World"));
        assert_eq!(r.user_named, Some(true));
        std::fs::remove_dir_all(&tmp).ok();
    }

    fn tempdir_unique() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ghcp-mon-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
