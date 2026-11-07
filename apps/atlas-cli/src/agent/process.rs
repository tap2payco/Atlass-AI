//! Process containment for `run_command`.
//!
//! - `ProcessTree`: dropping it (timeout, cancel, or the command finishing)
//!   ends every process the command started, so background processes do not
//!   outlive it.
//!   - Windows: a Job Object created with kill-on-close; the shell is
//!     assigned to it, so everything the shell starts is in the job too. A
//!     process started in the instant between spawn and assignment can
//!     escape; the `restricted` path avoids that by spawning suspended.
//!   - Unix: the shell is started as the leader of its own process group
//!     (`process_group(0)`), and the guard sends SIGKILL to the group.
//! - `restricted` (Windows): spawn with a restricted token, suspended, in a
//!   kill-on-close job. `policy.rs` says what that does and does not stop.
//! - `scrub_secrets` / `command_env`: remove environment variables whose
//!   names look like credentials, so a command (or a prompt-injected one)
//!   cannot read them.

#[cfg(windows)]
pub mod restricted;

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};

use tokio::process::{Child, Command};

/// Variables matching these name parts are removed from command environments.
const SECRET_PARTS: &[&str] = &[
    "KEY", "APIKEY", "TOKEN", "TOKENS", "SECRET", "SECRETS", "PASSWORD", "PASSWD", "CREDENTIAL",
    "CREDENTIALS",
];

/// Comma-separated variable names to keep even though they look secret.
pub const KEEP_ENV_VAR: &str = "ATLAS_COMMAND_KEEP_ENV";

/// Whether `name` looks like it holds a credential: one of its `_`/`-`
/// separated parts is KEY, TOKEN, SECRET, PASSWORD, … (case-insensitive).
pub fn is_secret_name(name: &str) -> bool {
    name.split(['_', '-'])
        .any(|part| SECRET_PARTS.iter().any(|s| part.eq_ignore_ascii_case(s)))
}

fn kept_names() -> HashSet<String> {
    std::env::var(KEEP_ENV_VAR)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn should_scrub(name: &str, keep: &HashSet<String>) -> bool {
    is_secret_name(name) && !keep.contains(&name.to_ascii_uppercase())
}

/// Remove secret-looking variables from `command`'s environment and return
/// the names removed (never the values).
pub fn scrub_secrets(command: &mut Command) -> Vec<String> {
    let keep = kept_names();
    let mut removed = Vec::new();
    for (name, _) in std::env::vars_os() {
        let Some(text) = name.to_str() else { continue };
        if should_scrub(text, &keep) {
            command.env_remove(OsStr::new(text));
            removed.push(text.to_string());
        }
    }
    removed.sort();
    removed
}

/// This process's environment without secret-looking variables, plus
/// `extra` (which replaces inherited values of the same name). Returns the
/// variables and the names removed (never the values).
pub fn command_env(extra: &[(&str, &str)]) -> (Vec<(OsString, OsString)>, Vec<String>) {
    let keep = kept_names();
    let mut removed = Vec::new();
    let mut vars: Vec<(OsString, OsString)> = Vec::new();
    for (name, value) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if name.to_str().is_some_and(|t| should_scrub(t, &keep)) {
            removed.push(text.into_owned());
        } else if !extra.iter().any(|(k, _)| text.eq_ignore_ascii_case(k)) {
            vars.push((name, value));
        }
    }
    vars.extend(extra.iter().map(|(k, v)| (OsString::from(k), OsString::from(v))));
    removed.sort();
    (vars, removed)
}

/// Kills the child's whole process tree when dropped.
pub struct ProcessTree {
    #[cfg(windows)]
    job: Option<windows::Job>,
    /// The child's process group (its pid), when it was spawned as a group leader.
    #[cfg(unix)]
    group: Option<i32>,
}

impl ProcessTree {
    /// Contain `child` (just spawned) and its future descendants: on Windows
    /// by putting it in a kill-on-close job; on Unix the child must have been
    /// spawned with `process_group(0)`, and its group is killed on drop.
    pub fn contain(child: &Child) -> Self {
        #[cfg(windows)]
        {
            let job = windows::Job::kill_on_close().filter(|job| match child.raw_handle() {
                Some(handle) => job.assign(handle),
                None => false,
            });
            if job.is_none() {
                tracing::warn!("could not put the command in a job object; only the shell will be stopped");
            }
            Self { job }
        }
        #[cfg(unix)]
        {
            Self { group: child.id().and_then(|id| i32::try_from(id).ok()).filter(|id| *id > 0) }
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = child;
            Self {}
        }
    }

    pub fn is_contained(&self) -> bool {
        #[cfg(windows)]
        {
            self.job.is_some()
        }
        #[cfg(unix)]
        {
            self.group.is_some()
        }
        #[cfg(not(any(windows, unix)))]
        {
            false
        }
    }
}

#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        if let Some(group) = self.group {
            // SAFETY: killpg only sends a signal; ESRCH (the group is already gone) is fine.
            unsafe {
                libc::killpg(group, libc::SIGKILL);
            }
        }
    }
}

#[cfg(windows)]
pub(crate) mod windows {
    use std::ffi::c_void;
    use std::os::windows::io::RawHandle;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct Job(HANDLE);

    // The handle is only used to assign processes and is closed once, on drop.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn kill_on_close() -> Option<Self> {
            // SAFETY: plain Win32 calls on a handle we own; the info struct
            // outlives the call and its size is passed exactly.
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return None;
                }
                let job = Job(handle);
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    &info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION as *const c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                (ok != 0).then_some(job)
            }
        }

        pub fn assign(&self, process: RawHandle) -> bool {
            // SAFETY: `process` is a live process handle owned by the caller.
            unsafe { AssignProcessToJobObject(self.0, process as HANDLE) != 0 }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // Closing the last handle to a kill-on-close job ends every process in it.
            // SAFETY: we own the handle and close it exactly once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_env_drops_secrets_and_adds_extras() {
        let name = format!("ATLAS_ENV_TEST_{}_SECRET", std::process::id());
        std::env::set_var(&name, "hidden");
        let (vars, removed) = command_env(&[("ATLAS_SANDBOX_NETWORK", "0")]);
        std::env::remove_var(&name);
        assert!(removed.contains(&name));
        assert!(!vars.iter().any(|(k, _)| k.to_string_lossy() == name));
        let network: Vec<_> = vars.iter().filter(|(k, _)| k == "ATLAS_SANDBOX_NETWORK").map(|(_, v)| v).collect();
        assert_eq!(network, [&OsString::from("0")]);
    }

    #[test]
    fn recognises_secret_names_without_false_positives() {
        for name in [
            "ATLAS_FIREWORKS_API_KEY", "OPENAI_API_KEY", "GITHUB_TOKEN", "NPM_TOKEN",
            "AWS_SECRET_ACCESS_KEY", "DB_PASSWORD", "my-app-secret", "GOOGLE_APPLICATION_CREDENTIALS",
        ] {
            assert!(is_secret_name(name), "{name} should be secret");
        }
        for name in ["PATH", "HOME", "PWD", "KEYBOARD_LAYOUT", "RUST_LOG", "SSH_AUTH_SOCK", "MONKEY"] {
            assert!(!is_secret_name(name), "{name} should not be secret");
        }
    }
}
