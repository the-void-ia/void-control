//! Daemon address discovery and bearer-token resolution for the void-box
//! HTTP transport.
//!
//! void-box now defaults its daemon listener to AF_UNIX at mode `0o600`, with
//! TCP available as an opt-in `--listen tcp://host:port` mode that requires a
//! bearer token. void-control mirrors that contract: when no daemon URL is
//! configured, it auto-discovers the same socket the daemon advertises on the
//! same uid; when an operator points it at TCP, it injects the same token the
//! daemon expects.
//!
//! # Lockstep contract with void-box
//!
//! [`default_unix_socket_path`] reproduces the exact path-discovery chain
//! implemented by `voidbox::daemon_listen::default_unix_socket_path` (in the
//! void-box repo). The two implementations are intentionally duplicated rather
//! than shared via a crate dependency: pulling the `void-box` library would
//! drag heavy Linux-only deps (`kvm-ioctls`, `kvm-bindings`, `vm-memory`,
//! `linux-loader`) into the control plane. Instead, the chain is small enough
//! to vendor — but if either side drifts, same-uid clients silently fail to
//! connect because server and client disagree about where the socket lives.
//! Treat this file as the contract surface: any change here must be reflected
//! in the daemon, and vice versa.
//!
//! Resolution order, in both implementations:
//! 1. `$XDG_RUNTIME_DIR/voidbox.sock` if the directory is writable.
//! 2. `$TMPDIR/voidbox-$UID.sock` if the directory is writable.
//! 3. `/tmp/voidbox-$UID.sock` as the final fallback.
//!
//! The per-uid suffix on the `$TMPDIR` and `/tmp` legs avoids cross-account
//! path collisions on shared hosts; `$XDG_RUNTIME_DIR` is per-user already so
//! a bare `voidbox.sock` is fine there.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Environment variable consulted before falling back to a generated token.
pub const DAEMON_TOKEN_ENV: &str = "VOIDBOX_DAEMON_TOKEN";

/// Environment variable that points at a `0o600` file containing the token.
pub const DAEMON_TOKEN_FILE_ENV: &str = "VOIDBOX_DAEMON_TOKEN_FILE";

/// Errors from daemon-address or token resolution.
#[derive(Debug)]
pub enum DaemonAddressError {
    /// The configured TCP daemon URL has no token configured by any of the
    /// supported sources. Surfaced at construction time so a misconfigured
    /// deployment fails loudly instead of dialing and discovering via 401.
    MissingTcpToken { url: String, searched: Vec<String> },
    /// A token file path was set but could not be read, had loose permissions,
    /// or was empty.
    TokenFileError { path: PathBuf, detail: String },
}

impl std::fmt::Display for DaemonAddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonAddressError::MissingTcpToken { url, searched } => {
                write!(
                    f,
                    "TCP daemon URL {url:?} requires a bearer token; none found in any of: {}",
                    searched.join(", ")
                )
            }
            DaemonAddressError::TokenFileError { path, detail } => {
                write!(f, "token file {}: {detail}", path.display())
            }
        }
    }
}

impl std::error::Error for DaemonAddressError {}

/// Discover the default AF_UNIX socket path the daemon listens on.
///
/// This MUST stay aligned with `voidbox::daemon_listen::default_unix_socket_path`
/// (in the void-box repo). See the module-level docs for the lockstep
/// contract.
///
/// We do not depend on `libc` directly to test write-access (the daemon side
/// uses `access(2)` for kernel-correct semantics including ACLs and mount
/// flags). Instead we approximate via `Path::is_dir()` plus a probe-create
/// of a uniquely-named file with `O_CREAT | O_EXCL`. The two checks may
/// disagree in exotic edge cases (an ACL-restricted dir that fails the kernel
/// check but where `O_CREAT|O_EXCL` happens to also fail with `EACCES` is
/// handled correctly; an ACL-restricted dir where the probe somehow
/// succeeds while a real socket bind fails would fall through). On a normal
/// developer host the two are equivalent and we keep the dependency surface
/// small.
pub fn default_unix_socket_path() -> PathBuf {
    if let Some(path) = dir_socket("XDG_RUNTIME_DIR", "voidbox.sock") {
        return path;
    }
    let uid = current_uid();
    let per_uid = format!("voidbox-{uid}.sock");
    if let Some(path) = dir_socket("TMPDIR", &per_uid) {
        return path;
    }
    PathBuf::from("/tmp").join(per_uid)
}

fn dir_socket(env_var: &str, file_name: &str) -> Option<PathBuf> {
    let raw = std::env::var(env_var).ok()?;
    let dir = PathBuf::from(raw);
    if dir.as_os_str().is_empty() {
        return None;
    }
    if !is_writable_dir(&dir) {
        return None;
    }
    Some(dir.join(file_name))
}

/// Best-effort writability probe: create a uniquely named file with
/// `create_new(true)` (O_CREAT|O_EXCL) and immediately remove it. Returns
/// `false` for non-directories, non-existent paths, and any I/O error.
fn is_writable_dir(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_dir() {
        return false;
    }
    let probe = path.join(probe_file_name());
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let writable = match options.open(&probe) {
        Ok(file) => {
            // sync_all() not needed — we are about to unlink it. Drop closes the fd.
            drop(file);
            true
        }
        Err(_) => false,
    };
    let _ = fs::remove_file(&probe);
    writable
}

static PROBE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn probe_file_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let nonce = PROBE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(".void-control-probe-{}-{nanos}-{nonce}", std::process::id())
}

#[cfg(unix)]
fn current_uid() -> u32 {
    // SAFETY: `geteuid` is always safe; it returns the calling process uid.
    unsafe { libc_geteuid() }
}

#[cfg(unix)]
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

#[cfg(not(unix))]
fn current_uid() -> u32 {
    0
}

/// Compute the default `unix://<path>` daemon URL.
pub fn default_unix_url() -> String {
    format!("unix://{}", default_unix_socket_path().display())
}

/// Classification of a daemon URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonScheme {
    /// `unix:///abs/path/to/socket` — AF_UNIX transport, no auth header.
    Unix(PathBuf),
    /// `http://host:port` or bare `host:port` — TCP transport, requires token.
    Tcp(String),
}

/// Classify a configured daemon URL into a transport scheme.
///
/// Accepted shapes:
/// - `unix:///abs/path/voidbox.sock` — AF_UNIX. Relative paths are rejected
///   loudly the same way the daemon rejects them in
///   `voidbox::bin::voidbox::backend::build_transport`.
/// - `http://host:port` — TCP.
/// - bare `host:port` — back-compat alias for `http://host:port` (same as the
///   daemon CLI's bare-form back-compat). Normalized to the `http://` form
///   here so downstream parsing has a single shape to handle.
pub fn classify_daemon_url(url: &str) -> Result<DaemonScheme, String> {
    if let Some(rest) = url.strip_prefix("unix://") {
        if !rest.starts_with('/') {
            return Err(format!(
                "invalid daemon URL {url:?}: unix:// scheme requires an absolute socket path \
                 (e.g. unix:///run/user/1000/voidbox.sock); got {rest:?}"
            ));
        }
        let trimmed = rest.trim_end_matches('/');
        return Ok(DaemonScheme::Unix(PathBuf::from(trimmed)));
    }
    if url.starts_with("http://") {
        return Ok(DaemonScheme::Tcp(url.to_string()));
    }
    // Bare host:port form — normalize to http:// for downstream uniformity.
    Ok(DaemonScheme::Tcp(format!("http://{url}")))
}

/// Bearer-token resolution result. `None` means no token was configured by any
/// source; the caller decides whether that's acceptable (it is for AF_UNIX,
/// it is not for TCP).
#[derive(Debug, Clone)]
pub struct ResolvedToken(pub Option<String>);

/// Default path the daemon writes auto-generated TCP tokens to. Mirrors
/// `voidbox::daemon_listen::default_token_path` so the client and daemon
/// converge with no manual wiring on the same host.
pub fn default_token_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(dir);
        if !p.as_os_str().is_empty() {
            return p.join("voidbox").join("daemon-token");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home);
        if !p.as_os_str().is_empty() {
            return p.join(".config").join("voidbox").join("daemon-token");
        }
    }
    PathBuf::from("/tmp")
        .join(format!("voidbox-{}", current_uid()))
        .join("daemon-token")
}

/// Resolve a bearer token using the same precedence the void-box CLI client
/// applies, in order:
///
/// 1. `VOIDBOX_DAEMON_TOKEN_FILE` — explicit `0o600` file.
/// 2. `VOIDBOX_DAEMON_TOKEN` — explicit env var.
/// 3. `default_token_path()` (`$XDG_CONFIG_HOME/voidbox/daemon-token` or
///    `$HOME/.config/voidbox/daemon-token`) — implicit, written by the daemon
///    on first start when neither of the above is configured.
///
/// `Ok(ResolvedToken(Some(_)))` if a non-empty token was found; `Ok(ResolvedToken(None))`
/// if nothing resolved. Loose permissions on a token file (`mode & 0o077 != 0`)
/// fail closed with [`DaemonAddressError::TokenFileError`].
///
/// When `VOIDBOX_DAEMON_TOKEN_FILE` is set but the file is missing or
/// unreadable, this emits a `WARN`-level log and continues to lower-priority
/// sources. The implicit `default_token_path` legs stay silent on
/// `NotFound` because that is the expected pre-generation state on a fresh
/// machine.
pub fn resolve_tcp_token() -> Result<ResolvedToken, DaemonAddressError> {
    if let Ok(path_value) = std::env::var(DAEMON_TOKEN_FILE_ENV) {
        let path = PathBuf::from(&path_value);
        match read_token_file(&path) {
            Ok(token) => return Ok(ResolvedToken(Some(token))),
            Err(DaemonAddressError::TokenFileError { path, detail }) => {
                if is_missing_or_unreadable(&detail) {
                    eprintln!(
                        "WARN: {DAEMON_TOKEN_FILE_ENV} points at {} but the file is missing or unreadable: {detail}",
                        path.display()
                    );
                } else {
                    return Err(DaemonAddressError::TokenFileError { path, detail });
                }
            }
            Err(other) => return Err(other),
        }
    }
    if let Ok(value) = std::env::var(DAEMON_TOKEN_ENV) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(ResolvedToken(Some(trimmed.to_string())));
        }
    }
    let implicit = default_token_path();
    match read_token_file(&implicit) {
        Ok(token) => Ok(ResolvedToken(Some(token))),
        Err(DaemonAddressError::TokenFileError { detail, .. })
            if is_missing_or_unreadable(&detail) =>
        {
            // Silent: the daemon hasn't generated a token yet. The caller
            // surfaces the "no token configured" state.
            Ok(ResolvedToken(None))
        }
        Err(err) => Err(err),
    }
}

fn is_missing_or_unreadable(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.contains("no such file") || lower.contains("not found") || lower.contains("permission")
}

/// Read a bearer-token file, enforcing the same `0o600`-style owner-only
/// permission check the daemon applies. Loose modes are rejected up front
/// rather than silently sent over the wire.
pub fn read_token_file(path: &Path) -> Result<String, DaemonAddressError> {
    let metadata = fs::metadata(path).map_err(|err| DaemonAddressError::TokenFileError {
        path: path.to_path_buf(),
        detail: err.to_string(),
    })?;
    require_token_file_perms(path, &metadata)?;
    let raw = fs::read_to_string(path).map_err(|err| DaemonAddressError::TokenFileError {
        path: path.to_path_buf(),
        detail: err.to_string(),
    })?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        return Err(DaemonAddressError::TokenFileError {
            path: path.to_path_buf(),
            detail: "token file is empty".into(),
        });
    }
    Ok(trimmed)
}

#[cfg(unix)]
fn require_token_file_perms(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), DaemonAddressError> {
    use std::os::unix::fs::MetadataExt;
    let mode = metadata.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(DaemonAddressError::TokenFileError {
            path: path.to_path_buf(),
            detail: format!(
                "token file mode is 0o{mode:03o}, must not be group/other accessible \
                 (typical fix: chmod 0600 {})",
                path.display()
            ),
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_token_file_perms(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), DaemonAddressError> {
    Ok(())
}

/// For tests: list the source labels searched by [`resolve_tcp_token`], in
/// order. Used in [`DaemonAddressError::MissingTcpToken`] error messages so
/// operators see exactly what was attempted.
pub fn token_search_labels() -> Vec<String> {
    vec![
        DAEMON_TOKEN_FILE_ENV.to_string(),
        DAEMON_TOKEN_ENV.to_string(),
        default_token_path().display().to_string(),
    ]
}

/// Convenience: write `contents` to `path` with mode `0o600`, creating parents.
/// Used by tests; not part of the resolve path because the daemon owns
/// generating the implicit token.
#[cfg(all(test, unix))]
pub(crate) fn write_owner_only_file(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let _ = fs::remove_file(path);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env mutation is process-global; serialize tests that set/unset env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        // Recover from poisoning so a single failing test doesn't cascade
        // through the rest of the env-mutating tests. We restore env on the
        // way out regardless of the closure's panic-safety, so
        // `AssertUnwindSafe` is the right call here.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let saved: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            match v {
                Some(value) => std::env::set_var(k, value),
                None => std::env::remove_var(k),
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        for (k, v) in saved {
            match v {
                Some(value) => std::env::set_var(k, value),
                None => std::env::remove_var(k),
            }
        }
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    fn unique_tmp_dir(label: &str) -> PathBuf {
        // Use `/tmp` directly rather than `env::temp_dir()`. The latter
        // resolves through `TMPDIR`, which the tests in this module mutate;
        // a precomputed temp path can otherwise nest inside a previously-set
        // tempdir and produce cross-test interference. `/tmp` is stable
        // across the test suite and short enough to keep AF_UNIX paths
        // under SUN_LEN on macOS.
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p =
            PathBuf::from("/tmp").join(format!("vc-test-{label}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn discover_uses_xdg_runtime_dir_when_writable() {
        let tmp = unique_tmp_dir("xdg");
        with_env(
            &[
                ("XDG_RUNTIME_DIR", Some(tmp.to_str().unwrap())),
                ("TMPDIR", None),
            ],
            || {
                let path = default_unix_socket_path();
                assert_eq!(path, tmp.join("voidbox.sock"));
            },
        );
        // Intentionally don't remove the dir: a parallel test reading
        // `env::temp_dir()` may still be using a path that points inside it
        // for the brief window where TMPDIR pointed here. /tmp is reaped by
        // the OS anyway.
    }

    #[test]
    fn discover_falls_through_to_tmpdir_when_xdg_missing() {
        let tmp = unique_tmp_dir("tmpdir");
        with_env(
            &[
                ("XDG_RUNTIME_DIR", None),
                ("TMPDIR", Some(tmp.to_str().unwrap())),
            ],
            || {
                let path = default_unix_socket_path();
                let uid = current_uid();
                assert_eq!(path, tmp.join(format!("voidbox-{uid}.sock")));
            },
        );
        // Intentionally don't remove the dir: a parallel test reading
        // `env::temp_dir()` may still be using a path that points inside it
        // for the brief window where TMPDIR pointed here. /tmp is reaped by
        // the OS anyway.
    }

    #[test]
    fn discover_falls_back_to_slash_tmp() {
        with_env(&[("XDG_RUNTIME_DIR", None), ("TMPDIR", None)], || {
            let path = default_unix_socket_path();
            let uid = current_uid();
            assert_eq!(
                path,
                PathBuf::from("/tmp").join(format!("voidbox-{uid}.sock"))
            );
        });
    }

    #[test]
    fn discover_skips_unwritable_xdg_runtime_dir() {
        with_env(
            &[
                (
                    "XDG_RUNTIME_DIR",
                    Some("/nonexistent-void-control-test-dir-xyzzy"),
                ),
                ("TMPDIR", None),
            ],
            || {
                let path = default_unix_socket_path();
                let uid = current_uid();
                assert_eq!(
                    path,
                    PathBuf::from("/tmp").join(format!("voidbox-{uid}.sock"))
                );
            },
        );
    }

    #[test]
    fn classify_unix_url_requires_absolute_path() {
        match classify_daemon_url("unix:///tmp/voidbox.sock").unwrap() {
            DaemonScheme::Unix(p) => assert_eq!(p, PathBuf::from("/tmp/voidbox.sock")),
            _ => panic!("expected unix"),
        }
        let err = classify_daemon_url("unix://relative/path").unwrap_err();
        assert!(err.contains("absolute socket path"));
    }

    #[test]
    fn classify_tcp_url_accepts_http_and_normalizes_bare() {
        match classify_daemon_url("http://127.0.0.1:43100").unwrap() {
            DaemonScheme::Tcp(s) => assert_eq!(s, "http://127.0.0.1:43100"),
            _ => panic!("expected tcp"),
        }
        match classify_daemon_url("127.0.0.1:43100").unwrap() {
            DaemonScheme::Tcp(s) => assert_eq!(s, "http://127.0.0.1:43100"),
            _ => panic!("expected tcp"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn token_file_rejected_when_world_readable() {
        use std::os::unix::fs::PermissionsExt;
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = unique_tmp_dir("loose-perm");
        let path = dir.join("token");
        fs::write(&path, "hunter2").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let err = read_token_file(&path).unwrap_err();
        match err {
            DaemonAddressError::TokenFileError { detail, .. } => {
                assert!(detail.contains("0o644") || detail.contains("group/other"));
            }
            _ => panic!("expected TokenFileError"),
        }
        // See discover_uses_xdg_runtime_dir_when_writable: don't remove.
    }

    #[cfg(unix)]
    #[test]
    fn token_file_accepted_at_0o600() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = unique_tmp_dir("ok-perm");
        let path = dir.join("token");
        write_owner_only_file(&path, "hunter2\n").unwrap();
        let token = read_token_file(&path).unwrap();
        assert_eq!(token, "hunter2");
        // See discover_uses_xdg_runtime_dir_when_writable: don't remove.
    }

    #[cfg(unix)]
    #[test]
    fn resolve_tcp_token_prefers_env_file_over_env_var_and_implicit() {
        let dir = unique_tmp_dir("file-priority");
        let path = dir.join("token");
        write_owner_only_file(&path, "from-file").unwrap();
        with_env(
            &[
                (DAEMON_TOKEN_FILE_ENV, Some(path.to_str().unwrap())),
                (DAEMON_TOKEN_ENV, Some("from-env")),
                ("XDG_CONFIG_HOME", Some(dir.to_str().unwrap())),
            ],
            || {
                let resolved = resolve_tcp_token().unwrap();
                assert_eq!(resolved.0.as_deref(), Some("from-file"));
            },
        );
        // See discover_uses_xdg_runtime_dir_when_writable: don't remove.
    }

    #[cfg(unix)]
    #[test]
    fn resolve_tcp_token_uses_env_var_when_no_file() {
        let dir = unique_tmp_dir("env-var");
        with_env(
            &[
                (DAEMON_TOKEN_FILE_ENV, None),
                (DAEMON_TOKEN_ENV, Some("from-env")),
                ("XDG_CONFIG_HOME", Some(dir.to_str().unwrap())),
            ],
            || {
                let resolved = resolve_tcp_token().unwrap();
                assert_eq!(resolved.0.as_deref(), Some("from-env"));
            },
        );
        // See discover_uses_xdg_runtime_dir_when_writable: don't remove.
    }

    #[cfg(unix)]
    #[test]
    fn resolve_tcp_token_falls_back_to_implicit_token_file() {
        let config_root = unique_tmp_dir("implicit");
        let token_path = config_root.join("voidbox").join("daemon-token");
        write_owner_only_file(&token_path, "from-implicit").unwrap();
        with_env(
            &[
                (DAEMON_TOKEN_FILE_ENV, None),
                (DAEMON_TOKEN_ENV, None),
                ("XDG_CONFIG_HOME", Some(config_root.to_str().unwrap())),
            ],
            || {
                let resolved = resolve_tcp_token().unwrap();
                assert_eq!(resolved.0.as_deref(), Some("from-implicit"));
            },
        );
        // See discover_uses_xdg_runtime_dir_when_writable: don't remove.
    }

    #[cfg(unix)]
    #[test]
    fn resolve_tcp_token_returns_none_when_nothing_configured() {
        // Point XDG_CONFIG_HOME at a fresh dir with no token file inside.
        let config_root = unique_tmp_dir("nothing");
        with_env(
            &[
                (DAEMON_TOKEN_FILE_ENV, None),
                (DAEMON_TOKEN_ENV, None),
                ("XDG_CONFIG_HOME", Some(config_root.to_str().unwrap())),
                ("HOME", Some(config_root.to_str().unwrap())),
            ],
            || {
                let resolved = resolve_tcp_token().unwrap();
                assert!(resolved.0.is_none());
            },
        );
        // See discover_uses_xdg_runtime_dir_when_writable: don't remove.
    }
}
