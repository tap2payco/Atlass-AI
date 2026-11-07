//! Interactive terminals for the Atlas app: a real shell in the project
//! folder, shown beside the chat (like the terminal panel in Claude's and
//! Codex's apps).
//!
//! On Windows the shell runs in a pseudo-console (ConPTY), so prompts,
//! colours, line editing and full-screen programs behave as in Windows
//! Terminal. Elsewhere it runs with piped stdio for now (no line editing).
//!
//! The browser reads output from a long-lived HTTP stream and sends
//! keystrokes as small POSTs (see `web_agent`). Recent output is kept, so a
//! reloaded page gets the screen back.
//!
//! These terminals are the user's own: they run with the user's rights and
//! environment, not the agent's sandbox. The server only listens on
//! 127.0.0.1 and refuses requests from other origins.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::broadcast;

/// Output kept for a page that (re)connects.
const BACKLOG_BYTES: usize = 256 * 1024;
/// At most this many terminals at once.
const MAX_TERMINALS: usize = 8;

pub struct Terminal {
    pub id: String,
    pub shell: String,
    writer: Mutex<File>,
    backlog: Mutex<Vec<u8>>,
    output: broadcast::Sender<Vec<u8>>,
    exited: AtomicBool,
    process: Process,
}

impl Terminal {
    /// Sends keystrokes or pasted text to the shell.
    pub fn write(&self, data: &[u8]) -> std::io::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.process.resize(cols.clamp(10, 500), rows.clamp(2, 300));
    }

    /// Output so far, then live output until the shell exits.
    pub fn subscribe(&self) -> (Vec<u8>, broadcast::Receiver<Vec<u8>>) {
        // Holding the backlog lock while subscribing means no chunk is
        // missed or sent twice between the two.
        let backlog = self.backlog.lock().unwrap();
        (backlog.clone(), self.output.subscribe())
    }

    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    pub fn kill(&self) {
        self.process.kill();
    }
}

fn terminals() -> &'static Mutex<HashMap<String, Arc<Terminal>>> {
    static TERMINALS: OnceLock<Mutex<HashMap<String, Arc<Terminal>>>> = OnceLock::new();
    TERMINALS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get(id: &str) -> Option<Arc<Terminal>> {
    terminals().lock().unwrap().get(id).cloned()
}

pub fn close(id: &str) -> bool {
    match terminals().lock().unwrap().remove(id) {
        Some(terminal) => {
            terminal.kill();
            true
        }
        None => false,
    }
}

/// The shell to start: `ATLAS_TERMINAL_SHELL`, else PowerShell on Windows
/// and `$SHELL` (or `/bin/sh`) elsewhere.
fn shell_command() -> String {
    if let Ok(shell) = std::env::var("ATLAS_TERMINAL_SHELL") {
        if !shell.trim().is_empty() {
            return shell;
        }
    }
    if cfg!(windows) {
        "powershell.exe -NoLogo".into()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

/// Starts a shell in `cwd`.
pub fn open(cwd: &Path, cols: u16, rows: u16) -> Result<Arc<Terminal>, String> {
    {
        let mut all = terminals().lock().unwrap();
        all.retain(|_, t| !t.has_exited());
        if all.len() >= MAX_TERMINALS {
            return Err(format!("At most {MAX_TERMINALS} terminals can be open. Close one first."));
        }
    }
    let shell = shell_command();
    let (process, reader, writer) =
        Process::spawn(&shell, cwd, cols.clamp(10, 500), rows.clamp(2, 300)).map_err(|e| format!("could not start {shell}: {e}"))?;
    let (output, _) = broadcast::channel(1024);
    let terminal = Arc::new(Terminal {
        id: new_id(),
        shell,
        writer: Mutex::new(writer),
        backlog: Mutex::new(Vec::new()),
        output,
        exited: AtomicBool::new(false),
        process,
    });

    let pump = terminal.clone();
    std::thread::Builder::new()
        .name(format!("terminal-{}", terminal.id))
        .spawn(move || pump_output(pump, reader))
        .map_err(|e| e.to_string())?;

    watch_exit(&terminal);
    terminals().lock().unwrap().insert(terminal.id.clone(), terminal.clone());
    Ok(terminal)
}

fn new_id() -> String {
    use std::sync::atomic::AtomicU64;
    use std::time::{SystemTime, UNIX_EPOCH};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    format!("t{:x}{:04x}", nanos & 0xffff_ffff_ffff, NEXT.fetch_add(1, Ordering::SeqCst) & 0xffff)
}

fn pump_output(terminal: Arc<Terminal>, mut reader: File) {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let chunk = buf[..n].to_vec();
                let mut backlog = terminal.backlog.lock().unwrap();
                backlog.extend_from_slice(&chunk);
                if backlog.len() > BACKLOG_BYTES {
                    let excess = backlog.len() - BACKLOG_BYTES;
                    backlog.drain(..excess);
                }
                let _ = terminal.output.send(chunk);
            }
        }
    }
    terminal.exited.store(true, Ordering::SeqCst);
    // Wakes readers so their streams end.
    let _ = terminal.output.send(Vec::new());
}

#[cfg(windows)]
use conpty::Process;

#[cfg(windows)]
mod conpty {
    use std::ffi::c_void;
    use std::fs::File;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{FromRawHandle, RawHandle};
    use std::path::Path;
    use std::sync::Mutex;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
    };
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList, TerminateProcess,
        UpdateProcThreadAttribute, WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
        INFINITE, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
        STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };

    use crate::agent::process::windows::Job;

    /// A shell attached to a pseudo-console.
    pub struct Process {
        console: Mutex<Option<HPCON>>,
        process: HANDLE,
        _job: Option<Job>,
    }

    // The handles are used from several threads only through Win32 calls
    // that are safe to make concurrently; the console handle is behind a
    // mutex and closed once.
    unsafe impl Send for Process {}
    unsafe impl Sync for Process {}

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text).encode_wide().chain(std::iter::once(0)).collect()
    }

    impl Process {
        /// Returns the process, the console's output (to read) and input
        /// (to write).
        pub fn spawn(command: &str, cwd: &Path, cols: u16, rows: u16) -> std::io::Result<(Self, File, File)> {
            let err = std::io::Error::last_os_error;
            // SAFETY: plain Win32 calls. Every handle created here is either
            // closed here or handed to exactly one owner (File, Process).
            // The attribute list buffer outlives CreateProcessW.
            unsafe {
                let (mut in_read, mut in_write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
                let (mut out_read, mut out_write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
                if CreatePipe(&mut in_read, &mut in_write, std::ptr::null(), 0) == 0 {
                    return Err(err());
                }
                if CreatePipe(&mut out_read, &mut out_write, std::ptr::null(), 0) == 0 {
                    let e = err();
                    CloseHandle(in_read);
                    CloseHandle(in_write);
                    return Err(e);
                }
                let mut console: HPCON = 0;
                let size = COORD { X: cols as i16, Y: rows as i16 };
                let hr = CreatePseudoConsole(size, in_read, out_write, 0, &mut console);
                // The console holds its own references to these ends.
                CloseHandle(in_read);
                CloseHandle(out_write);
                let input = File::from_raw_handle(in_write as RawHandle);
                let output = File::from_raw_handle(out_read as RawHandle);
                if hr < 0 {
                    return Err(std::io::Error::other(format!("CreatePseudoConsole failed (0x{hr:08x})")));
                }

                let mut list_size = 0usize;
                InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut list_size);
                let mut list_buf = vec![0u8; list_size];
                let list = list_buf.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
                if InitializeProcThreadAttributeList(list, 1, 0, &mut list_size) == 0 {
                    let e = err();
                    ClosePseudoConsole(console);
                    return Err(e);
                }
                // The attribute's value is the console handle itself.
                if UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                    console as *const c_void,
                    std::mem::size_of::<HPCON>(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                ) == 0
                {
                    let e = err();
                    DeleteProcThreadAttributeList(list);
                    ClosePseudoConsole(console);
                    return Err(e);
                }

                let mut startup: STARTUPINFOEXW = std::mem::zeroed();
                startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
                startup.lpAttributeList = list;
                // Without this the shell inherits the server's own std
                // handles (redirected when run from a script or service)
                // and writes there instead of to the pseudo-console.
                startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
                let mut info: PROCESS_INFORMATION = std::mem::zeroed();
                let mut command_line = wide(command);
                let cwd = wide(&cwd.to_string_lossy());
                let ok = CreateProcessW(
                    std::ptr::null(),
                    command_line.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                    std::ptr::null(),
                    cwd.as_ptr(),
                    &startup.StartupInfo,
                    &mut info,
                );
                DeleteProcThreadAttributeList(list);
                if ok == 0 {
                    let e = err();
                    ClosePseudoConsole(console);
                    return Err(e);
                }
                CloseHandle(info.hThread);

                // The shell and everything it starts die with the server.
                let job = Job::kill_on_close();
                if let Some(job) = &job {
                    job.assign(info.hProcess as RawHandle);
                }

                let process = Process { console: Mutex::new(Some(console)), process: info.hProcess, _job: job };
                Ok((process, output, input))
            }
        }

        pub fn resize(&self, cols: u16, rows: u16) {
            if let Some(console) = *self.console.lock().unwrap() {
                // SAFETY: the console is open while it is in the mutex.
                unsafe {
                    ResizePseudoConsole(console, COORD { X: cols as i16, Y: rows as i16 });
                }
            }
        }

        pub fn kill(&self) {
            // SAFETY: the process handle stays open until drop.
            unsafe {
                TerminateProcess(self.process, 1);
            }
        }

        /// Blocks until the shell exits, then closes the console so the
        /// output pipe ends and the reader stops.
        pub fn wait_and_close(&self) {
            // SAFETY: as above.
            unsafe {
                WaitForSingleObject(self.process, INFINITE);
            }
            self.close_console();
        }

        fn close_console(&self) {
            if let Some(console) = self.console.lock().unwrap().take() {
                // SAFETY: taken out of the mutex, so closed exactly once.
                unsafe { ClosePseudoConsole(console) };
            }
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            self.close_console();
            // SAFETY: owned handle, closed once.
            unsafe {
                CloseHandle(self.process);
            }
        }
    }
}

#[cfg(not(windows))]
use piped::Process;

#[cfg(not(windows))]
mod piped {
    use std::fs::File;
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::sync::Mutex;

    /// A shell with piped stdio (no pseudo-terminal yet).
    pub struct Process {
        child: Mutex<Child>,
    }

    impl Process {
        pub fn spawn(command: &str, cwd: &Path, _cols: u16, _rows: u16) -> std::io::Result<(Self, File, File)> {
            let mut parts = command.split_whitespace();
            let program = parts.next().unwrap_or("/bin/sh");
            let mut child = Command::new(program)
                .args(parts)
                .arg("-i")
                .current_dir(cwd)
                .env("TERM", "dumb")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            let stdin = child.stdin.take().expect("piped");
            let stdout = child.stdout.take().expect("piped");
            // stderr is merged into the same stream by a copier thread.
            let stderr = child.stderr.take().expect("piped");
            // SAFETY: the descriptors were just taken from the child and are
            // owned exclusively by the returned Files.
            let (reader, writer) = unsafe {
                (File::from_raw_fd(stdout.into_raw_fd()), File::from_raw_fd(stdin.into_raw_fd()))
            };
            let _ = stderr;
            Ok((Process { child: Mutex::new(child) }, reader, writer))
        }

        pub fn resize(&self, _cols: u16, _rows: u16) {}

        pub fn kill(&self) {
            let _ = self.child.lock().unwrap().kill();
        }

        pub fn wait_and_close(&self) {
            let _ = self.child.lock().unwrap().wait();
        }
    }
}

/// Watches a terminal's shell and closes its console when it exits.
fn watch_exit(terminal: &Arc<Terminal>) {
    let watched = terminal.clone();
    let _ = std::thread::Builder::new()
        .name(format!("terminal-wait-{}", terminal.id))
        .spawn(move || watched.process.wait_and_close());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[cfg(windows)]
    #[test]
    fn a_shell_runs_in_a_pseudo_console_and_echoes_output() {
        std::env::set_var("ATLAS_TERMINAL_SHELL", "cmd.exe /Q /K");
        let dir = std::env::temp_dir();
        let terminal = open(&dir, 80, 24).expect("terminal starts");
        terminal.write(b"echo atlas-terminal-ok\r\n").unwrap();

        let deadline = Instant::now() + Duration::from_secs(15);
        let mut seen = String::new();
        while Instant::now() < deadline && !seen.contains("atlas-terminal-ok\r\n") {
            std::thread::sleep(Duration::from_millis(100));
            seen = String::from_utf8_lossy(&terminal.subscribe().0).into_owned();
        }
        assert!(seen.contains("atlas-terminal-ok"), "output was: {seen:?}");

        terminal.resize(100, 30);
        terminal.write(b"exit\r\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline && !terminal.has_exited() {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(terminal.has_exited(), "the reader ends once the shell exits");
        assert!(close(&terminal.id));
    }
}
