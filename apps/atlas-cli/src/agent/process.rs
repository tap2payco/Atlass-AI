//! Process containment for `run_command`.
//!
//! - `ProcessTree` (Windows): a Job Object created with kill-on-close. The
//!   command's shell is assigned to it, so everything the shell starts is in
//!   the job too, and dropping the guard (timeout, cancel, or the command
//!   finishing) ends all of them. Background processes a command starts do
//!   not outlive it. A process started in the instant between spawn and
//!   assignment can escape; Codex avoids that by spawning suspended.
//!   On other platforms the guard does nothing yet: only the shell is killed.
//! - `scrub_secrets`: removes environment variables whose names look like
//!   credentials, so a command (or a prompt-injected one) cannot read them.

use std::collections::HashSet;
use std::ffi::OsStr;

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

/// Remove secret-looking variables from `command`'s environment and return
/// the names removed (never the values).
pub fn scrub_secrets(command: &mut Command) -> Vec<String> {
    let keep: HashSet<String> = std::env::var(KEEP_ENV_VAR)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    let mut removed = Vec::new();
    for (name, _) in std::env::vars_os() {
        let Some(text) = name.to_str() else { continue };
        if is_secret_name(text) && !keep.contains(&text.to_ascii_uppercase()) {
            command.env_remove(OsStr::new(text));
            removed.push(text.to_string());
        }
    }
    removed.sort();
    removed
}

/// Kills the child's whole process tree when dropped (Windows).
pub struct ProcessTree {
    #[cfg(windows)]
    job: Option<windows::Job>,
}

impl ProcessTree {
    /// Put `child` (just spawned) and its future descendants in a kill-on-close job.
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
        #[cfg(not(windows))]
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
        #[cfg(not(windows))]
        {
            false
        }
    }
}

#[cfg(windows)]
mod windows {
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
            // SAFETY: `process` is a live process handle owned by tokio's Child.
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
