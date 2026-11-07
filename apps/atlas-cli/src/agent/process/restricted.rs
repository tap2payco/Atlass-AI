//! Windows: start a command with a restricted token.
//!
//! The token is the user's own token passed through `CreateRestrictedToken`
//! with `DISABLE_MAX_PRIVILEGE`, which deletes every privilege except
//! SeChangeNotifyPrivilege (it does not remove groups or narrow file
//! access). With `Integrity::Low` the token is also set to Low integrity, so
//! the default "no write up" rule stops the command from writing ordinary
//! (Medium) files and registry keys. The process is created suspended,
//! assigned to a kill-on-close job, then resumed, so nothing it starts can
//! escape the job. Only the three standard handles are inherited.
//!
//! A restricted version of the caller's own token does not need
//! SeAssignPrimaryTokenPrivilege, so this works for ordinary users.

use std::ffi::{c_void, OsStr, OsString};
use std::fs::File;
use std::io::{self, Read};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Output};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, WAIT_OBJECT_0};
use windows_sys::Win32::Security::{
    CreateRestrictedToken, CreateWellKnownSid, GetLengthSid, SetTokenInformation, TokenIntegrityLevel,
    WinLowLabelSid, DISABLE_MAX_PRIVILEGE, SECURITY_ATTRIBUTES, SECURITY_MAX_SID_SIZE, SID_AND_ATTRIBUTES, TOKEN_ADJUST_DEFAULT, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessAsUserW, DeleteProcThreadAttributeList, GetCurrentProcess, GetExitCodeProcess,
    InitializeProcThreadAttributeList, OpenProcessToken, ResumeThread, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, INFINITE, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

use super::windows::Job;

/// `SE_GROUP_INTEGRITY` (winnt.h); windows-sys keeps it behind an unrelated feature.
const SE_GROUP_INTEGRITY: u32 = 0x20;

/// The integrity level of the restricted token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrity {
    /// The caller's (normally Medium).
    Unchanged,
    /// Low: no writes to Medium-integrity files, keys or processes.
    Low,
}

/// A command started with a restricted token. Dropping it ends the command
/// and everything it started.
pub struct RestrictedChild {
    process: Arc<OwnedHandle>,
    stdout: Option<File>,
    stderr: Option<File>,
    /// `None` if the job could not be created; then only the shell is killed.
    job: Option<Job>,
}

fn last_error(what: &str) -> io::Error {
    let e = io::Error::last_os_error();
    io::Error::new(e.kind(), format!("{what}: {e}"))
}

fn check(ok: i32, what: &str) -> io::Result<()> {
    if ok != 0 { Ok(()) } else { Err(last_error(what)) }
}

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// # Safety
/// `handle` must be a valid handle the caller owns and does not close.
unsafe fn owned(handle: HANDLE) -> OwnedHandle {
    OwnedHandle::from_raw_handle(handle as _)
}

fn raw(handle: &impl AsRawHandle) -> HANDLE {
    handle.as_raw_handle() as HANDLE
}

fn restricted_token(integrity: Integrity) -> io::Result<OwnedHandle> {
    // SAFETY: Win32 calls with out-pointers to locals; returned handles are
    // wrapped in OwnedHandle immediately, and the SID buffer outlives its use.
    unsafe {
        let mut base: HANDLE = std::ptr::null_mut();
        check(
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT,
                &mut base,
            ),
            "OpenProcessToken",
        )?;
        let base = owned(base);
        let mut token: HANDLE = std::ptr::null_mut();
        check(
            CreateRestrictedToken(
                raw(&base),
                DISABLE_MAX_PRIVILEGE,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                &mut token,
            ),
            "CreateRestrictedToken",
        )?;
        let token = owned(token);
        if integrity == Integrity::Low {
            // u32 storage keeps the SID 4-byte aligned.
            let mut sid = [0u32; (SECURITY_MAX_SID_SIZE as usize).div_ceil(4)];
            let mut size = SECURITY_MAX_SID_SIZE;
            let psid = sid.as_mut_ptr() as *mut c_void;
            check(CreateWellKnownSid(WinLowLabelSid, std::ptr::null_mut(), psid, &mut size), "CreateWellKnownSid")?;
            let label = TOKEN_MANDATORY_LABEL {
                Label: SID_AND_ATTRIBUTES { Sid: psid, Attributes: SE_GROUP_INTEGRITY },
            };
            check(
                SetTokenInformation(
                    raw(&token),
                    TokenIntegrityLevel,
                    &label as *const TOKEN_MANDATORY_LABEL as *const c_void,
                    std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(psid),
                ),
                "SetTokenInformation(integrity)",
            )?;
        }
        Ok(token)
    }
}

/// A pipe whose write end is inheritable and whose read end is not.
fn pipe() -> io::Result<(File, OwnedHandle)> {
    let attrs = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    // SAFETY: out-pointers to locals; both handles are wrapped at once.
    unsafe {
        let (mut read, mut write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
        check(CreatePipe(&mut read, &mut write, &attrs, 0), "CreatePipe")?;
        let (read, write) = (owned(read), owned(write));
        check(SetHandleInformation(raw(&read), HANDLE_FLAG_INHERIT, 0), "SetHandleInformation")?;
        Ok((File::from(read), write))
    }
}

/// `NAME=value\0…\0\0`, sorted case-insensitively as Windows expects.
fn environment_block(env: &[(OsString, OsString)]) -> Vec<u16> {
    let mut vars: Vec<&(OsString, OsString)> = env.iter().collect();
    vars.sort_by_key(|(k, _)| k.to_string_lossy().to_uppercase());
    let mut block = Vec::new();
    for (k, v) in vars {
        block.extend(k.encode_wide());
        block.push(u16::from(b'='));
        block.extend(v.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

/// A proc-thread attribute list that limits inheritance to `handles`.
struct HandleList {
    buffer: Vec<usize>,
    // Kept alive: the list points into it until CreateProcess returns.
    _handles: Box<[HANDLE]>,
}

impl HandleList {
    fn new(handles: Box<[HANDLE]>) -> io::Result<Self> {
        // SAFETY: the first call only reports the size; the buffer is large
        // enough and pointer-aligned, and lives as long as the list.
        unsafe {
            let mut size = 0usize;
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
            let mut buffer = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
            let list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
            check(InitializeProcThreadAttributeList(list, 1, 0, &mut size), "InitializeProcThreadAttributeList")?;
            let this = Self { buffer, _handles: handles };
            check(
                UpdateProcThreadAttribute(
                    this.list(),
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    this._handles.as_ptr() as *const c_void,
                    std::mem::size_of_val(&*this._handles),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                ),
                "UpdateProcThreadAttribute",
            )?;
            Ok(this)
        }
    }

    fn list(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST
    }
}

impl Drop for HandleList {
    fn drop(&mut self) {
        // SAFETY: the list was initialised in `new`.
        unsafe { DeleteProcThreadAttributeList(self.list()) }
    }
}

/// The current directory to give a child: without the `\\?\` prefix, which cmd.exe rejects.
fn plain_dir(dir: &Path) -> OsString {
    let s = dir.as_os_str().to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => OsString::from(rest),
        _ => dir.as_os_str().to_owned(),
    }
}

impl RestrictedChild {
    /// Start `program` with the full `command_line` (argv[0] included), in
    /// `cwd`, with exactly `env`, under a restricted token.
    pub fn spawn(
        program: &Path,
        command_line: &OsStr,
        cwd: &Path,
        env: &[(OsString, OsString)],
        integrity: Integrity,
    ) -> io::Result<Self> {
        let token = restricted_token(integrity)?;
        let (stdout, stdout_w) = pipe()?;
        let (stderr, stderr_w) = pipe()?;
        let stdin: OwnedHandle = File::open("NUL")?.into();
        // SAFETY: a live handle we own.
        check(unsafe { SetHandleInformation(raw(&stdin), HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) }, "SetHandleInformation")?;

        let handles = HandleList::new(Box::new([raw(&stdin), raw(&stdout_w), raw(&stderr_w)]))?;
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = raw(&stdin);
        startup.StartupInfo.hStdOutput = raw(&stdout_w);
        startup.StartupInfo.hStdError = raw(&stderr_w);
        startup.lpAttributeList = handles.list();

        let program = wide(program.as_os_str());
        let mut command_line = wide(command_line);
        let cwd = wide(&plain_dir(cwd));
        let env = environment_block(env);
        let mut info = PROCESS_INFORMATION::default();
        // SAFETY: every pointer refers to a live, NUL-terminated local buffer
        // (the command line is mutable as the API requires); the handles in
        // the attribute list are live until after the call.
        unsafe {
            check(
                CreateProcessAsUserW(
                    raw(&token),
                    program.as_ptr(),
                    command_line.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                    CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                    env.as_ptr() as *const c_void,
                    cwd.as_ptr(),
                    &startup.StartupInfo,
                    &mut info,
                ),
                "CreateProcessAsUserW",
            )?;
        }
        // SAFETY: CreateProcess returned these handles to us.
        let (process, thread) = unsafe { (owned(info.hProcess), owned(info.hThread)) };
        // The child has its own copies now; ours must close so the pipes reach EOF.
        drop((handles, stdin, stdout_w, stderr_w));

        let job = Job::kill_on_close().filter(|job| job.assign(process.as_raw_handle()));
        if job.is_none() {
            tracing::warn!("could not put the command in a job object; only the shell will be stopped");
        }
        let child = Self { process: Arc::new(process), stdout: Some(stdout), stderr: Some(stderr), job };
        // SAFETY: a live thread handle we own.
        if unsafe { ResumeThread(raw(&thread)) } == u32::MAX {
            return Err(last_error("ResumeThread")); // dropping `child` ends the process
        }
        Ok(child)
    }

    pub fn is_contained(&self) -> bool {
        self.job.is_some()
    }

    /// Wait for the command to exit and collect its output.
    pub async fn wait_with_output(mut self) -> io::Result<Output> {
        fn drain(file: Option<File>) -> tokio::task::JoinHandle<io::Result<Vec<u8>>> {
            tokio::task::spawn_blocking(move || {
                let mut bytes = Vec::new();
                match file.map(|mut f| f.read_to_end(&mut bytes)) {
                    Some(Err(e)) if e.kind() != io::ErrorKind::BrokenPipe => Err(e),
                    _ => Ok(bytes),
                }
            })
        }
        fn joined<T>(r: Result<io::Result<T>, tokio::task::JoinError>) -> io::Result<T> {
            r.map_err(io::Error::other)?
        }
        let stdout = drain(self.stdout.take());
        let stderr = drain(self.stderr.take());
        let process = self.process.clone();
        let status = tokio::task::spawn_blocking(move || -> io::Result<ExitStatus> {
            // SAFETY: the handle stays open while this Arc is held.
            unsafe {
                if WaitForSingleObject(raw(&*process), INFINITE) != WAIT_OBJECT_0 {
                    return Err(last_error("WaitForSingleObject"));
                }
                let mut code = 0u32;
                check(GetExitCodeProcess(raw(&*process), &mut code), "GetExitCodeProcess")?;
                Ok(ExitStatus::from_raw(code))
            }
        });
        let status = joined(status.await)?;
        Ok(Output { status, stdout: joined(stdout.await)?, stderr: joined(stderr.await)? })
    }
}

impl Drop for RestrictedChild {
    fn drop(&mut self) {
        if self.job.is_none() {
            // SAFETY: a live process handle; failing on an exited process is harmless.
            unsafe {
                TerminateProcess(raw(&*self.process), 1);
            }
        }
        // Otherwise dropping the job ends the whole tree.
    }
}
