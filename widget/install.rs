//! What the installer does, for both faces: where Affinity lives, the install
//! steps, removal and launching — the original widget's `_run_install`,
//! `_perform_removal` and `_launch_app`, with its messages.

use crate::ui::tr;
use linexin_plugin_api::Host;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

pub const DOWNLOAD_URL: &str = "https://downloads.affinity.studio/Affinity%20x64.exe";

/// The downloaded installer, in the temporary directory.
const DOWNLOADED_EXE: &str = "Affinity_x64.exe";
const CONFIG_SCRIPT: &str = ".local/share/linexin/scripts/wine-affinity-config.sh";

/// One app of the suite: its folder under `AFFINITY_DIR`, its executable and
/// its menu entry.
#[derive(Debug, PartialEq, Eq)]
pub struct AppInfo {
    pub name: &'static str,
    pub folder: &'static str,
    pub exe: &'static str,
    pub desktop: &'static str,
}

/// The original `APP_REGISTRY`, in its order.
pub const APP_REGISTRY: [AppInfo; 4] = [
    AppInfo {
        name: "Affinity",
        folder: "Affinity",
        exe: "Affinity.exe",
        desktop: "affinity.desktop",
    },
    AppInfo {
        name: "Photo 2",
        folder: "Photo 2",
        exe: "Photo.exe",
        desktop: "photo-2.desktop",
    },
    AppInfo {
        name: "Designer 2",
        folder: "Designer 2",
        exe: "Designer.exe",
        desktop: "designer-2.desktop",
    },
    AppInfo {
        name: "Publisher 2",
        folder: "Publisher 2",
        exe: "Publisher.exe",
        desktop: "publisher-2.desktop",
    },
];

/// `os.path.expanduser("~")`: `$HOME`, else the account's home.
fn home() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(gtk::glib::home_dir)
}

/// `WINE_PREFIX`: `~/.WineAffinity`.
pub fn wine_prefix() -> PathBuf {
    home().join(".WineAffinity")
}

/// `AFFINITY_DIR`: where the suite's installer puts the apps.
pub fn affinity_dir() -> PathBuf {
    wine_prefix()
        .join("drive_c")
        .join("Program Files")
        .join("Affinity")
}

fn applications_dir() -> PathBuf {
    home().join(".local/share/applications")
}

/// The original `_is_installed`.
pub fn is_installed() -> bool {
    let dir = affinity_dir();
    dir.is_dir() && APP_REGISTRY.iter().any(|app| dir.join(app.folder).is_dir())
}

/// The original `_get_installed_apps`, in registry order.
pub fn installed_apps() -> Vec<&'static AppInfo> {
    let dir = affinity_dir();
    APP_REGISTRY
        .iter()
        .filter(|app| dir.join(app.folder).is_dir())
        .collect()
}

/// `tempfile.gettempdir()`: `$TMPDIR`, `$TEMP` or `$TMP`, else `/tmp`.
pub fn temp_dir() -> PathBuf {
    ["TMPDIR", "TEMP", "TMP"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|dir| dir.is_dir())
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// How the one privileged command (installing `wine-affinity`) is authorized.
#[derive(Clone, Copy)]
pub enum Privilege {
    /// The Center's shared sudo helper, as the Python widget's `sudo_manager`
    /// (GTK: the password was asked for already). Without the Center's
    /// services, plain `sudo`, as the original's fallback.
    Sudo(Option<Host>),
    /// `pkexec`, answered by the session's polkit agent — on LineXinBar the
    /// shell's own controller-friendly password panel.
    Polkit,
}

impl Privilege {
    fn wrapper(&self) -> String {
        match self {
            Privilege::Sudo(Some(host)) => host.sudo_wrapper_path(),
            Privilege::Sudo(None) => "sudo".into(),
            Privilege::Polkit => "pkexec".into(),
        }
    }

    /// The original Center locked its window while a command through the
    /// sudo wrapper ran.
    fn locks(&self) -> Option<Host> {
        match self {
            Privilege::Sudo(host) => *host,
            Privilege::Polkit => None,
        }
    }
}

/// What the install tells its page as it goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// A step began: the progress bar goes to `step / total`.
    Step { step: u32, label: String },
    /// The download moved the bar to this fraction.
    Fraction(f64),
}

/// The original `total_steps`: downloading adds one.
pub fn total_steps(downloading: bool) -> u32 {
    if downloading { 6 } else { 5 }
}

/// The original `_run_install`, on a worker thread: `Ok` when it went
/// through, else the message of the failure dialog (`str(e)` of what the
/// original raised). A sudo session is started here; the caller ends it.
pub fn run(
    exe: Option<PathBuf>,
    privilege: Privilege,
    report: &dyn Fn(Report),
) -> Result<(), String> {
    if let Privilege::Sudo(Some(host)) = privilege {
        host.sudo_start_session();
    }
    steps(exe, privilege, report)
}

fn steps(
    exe: Option<PathBuf>,
    privilege: Privilege,
    report: &dyn Fn(Report),
) -> Result<(), String> {
    let downloading = exe.is_none();
    let total = f64::from(total_steps(downloading));
    let mut step = 0;
    let begin = |step: u32, key: &str| {
        report(Report::Step {
            step,
            label: tr(key).replacen("{}", &step.to_string(), 1),
        });
    };

    // Download (optional)
    let exe = match exe {
        Some(exe) => exe,
        None => {
            step += 1;
            begin(step, "Step {}: Downloading installer...");
            let target = temp_dir().join(DOWNLOADED_EXE);
            let current = f64::from(step);
            download(DOWNLOAD_URL, &target, &|fraction| {
                report(Report::Fraction((current - 1.0 + fraction) / total));
            })?;
            target
        }
    };

    // Install Wine
    step += 1;
    begin(step, "Step {}: Installing Wine...");
    run_cmd(
        &format!(
            "{} pacman -Sy wine-affinity --noconfirm && \
             chmod 775 $HOME/.local/share/linexin/scripts/wine-affinity-config.sh",
            privilege.wrapper()
        ),
        privilege.locks(),
    )?;

    // Install required components
    step += 1;
    begin(step, "Step {}: Installing required components...");
    let code = answer_yes(&home().join(CONFIG_SCRIPT))?;
    if code != 0 {
        return Err(
            tr("Setting up the environment failed with exit code {}.").replacen(
                "{}",
                &code.to_string(),
                1,
            ),
        );
    }

    // Run the Affinity installer
    step += 1;
    begin(step, "Step {}: Installing Affinity...");
    run_cmd(
        &format!(
            "rum wine-affinity {} wine {}",
            shell_quote(wine_prefix().as_os_str().as_bytes()),
            shell_quote(exe.as_os_str().as_bytes())
        ),
        None,
    )?;

    // Create shortcuts
    step += 1;
    begin(step, "Step {}: Creating application shortcuts...");
    let shortcuts = home().join(".local/share/linexin/shortcuts");
    let applications = applications_dir();
    std::fs::create_dir_all(&applications).map_err(|error| os_error(&error, &[&applications]))?;
    let dir = affinity_dir();
    for app in &APP_REGISTRY {
        if dir.join(app.folder).is_dir() {
            let source = shortcuts.join(app.desktop);
            if source.is_file() {
                move_file(&source, &applications.join(app.desktop))?;
            }
        }
    }

    // Clean up
    step += 1;
    begin(step, "Step {}: Cleaning up...");
    if let Some(desktop) = gtk::glib::user_special_dir(gtk::glib::UserDirectory::Desktop) {
        let shortcut = desktop.join("Affinity.lnk");
        if shortcut.exists() {
            let _ = std::fs::remove_file(shortcut);
        }
    }
    let downloaded = temp_dir().join(DOWNLOADED_EXE);
    if downloaded.exists() {
        let _ = std::fs::remove_file(downloaded);
    }
    Ok(())
}

/// The original `urllib.request.urlretrieve` with its progress hook, done by
/// `curl`; failures are worded like urllib's (`HTTP Error 404: Not Found`,
/// `<urlopen error ...>`).
fn download(url: &str, target: &Path, progress: &dyn Fn(f64)) -> Result<(), String> {
    let mut child = Command::new("curl")
        .args([
            "--location",
            "--fail",
            "--progress-bar",
            "--write-out",
            "%{http_code}",
            "--output",
        ])
        .arg(target)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| os_error(&error, &[Path::new("curl")]))?;
    let mut stderr = child.stderr.take().expect("piped stderr");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let reader = std::thread::spawn(move || {
        let mut code = String::new();
        let _ = stdout.read_to_string(&mut code);
        code
    });

    // The bar is redrawn after each `\r`; anything else is curl talking.
    let mut meter = Meter::default();
    let mut buffer = [0u8; 4096];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                for fraction in meter.feed(&buffer[..read]) {
                    progress(fraction);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    meter.finish();
    let status = child.wait().map_err(|error| error.to_string())?;
    let code = reader.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let http = code.trim().parse::<u16>().unwrap_or(0);
    if status.code() == Some(22) && http >= 400 {
        return Err(format!("HTTP Error {http}: {}", http_reason(http)));
    }
    let said = meter
        .said
        .iter()
        .rev()
        .find_map(|line| line.strip_prefix("curl: "))
        .map(|line| match line.split_once(") ") {
            Some((number, rest)) if number.starts_with('(') => rest.to_owned(),
            _ => line.to_owned(),
        })
        .unwrap_or_else(|| match status.code() {
            Some(code) => format!("curl exited with status {code}"),
            None => format!("curl died with signal {}", status.signal().unwrap_or(0)),
        });
    Err(format!("<urlopen error {said}>"))
}

/// Reads curl's `--progress-bar` output: fractions from the bar, and the
/// lines it printed besides.
#[derive(Default)]
struct Meter {
    pending: Vec<u8>,
    said: Vec<String>,
}

impl Meter {
    fn feed(&mut self, bytes: &[u8]) -> Vec<f64> {
        let mut fractions = Vec::new();
        for &byte in bytes {
            if byte == b'\r' || byte == b'\n' {
                self.segment(&mut fractions);
            } else {
                self.pending.push(byte);
            }
        }
        fractions
    }

    fn finish(&mut self) {
        self.segment(&mut Vec::new());
    }

    fn segment(&mut self, fractions: &mut Vec<f64>) {
        let text = String::from_utf8_lossy(&self.pending).trim().to_owned();
        self.pending.clear();
        if text.is_empty() {
            return;
        }
        match bar_fraction(&text) {
            Some(fraction) => fractions.push(fraction),
            // The bar of a download of unknown size (`#=#=#`, `-=O=-`) says
            // nothing, as urllib's hook said nothing without a total.
            None if text.chars().all(|c| "#=O- ".contains(c)) => {}
            None => self.said.push(text),
        }
    }
}

/// `######    42.5%` → 0.425.
fn bar_fraction(text: &str) -> Option<f64> {
    let (bar, percent) = text.rsplit_once(' ').unwrap_or(("", text));
    if !bar.chars().all(|c| c == '#' || c == ' ') {
        return None;
    }
    let value: f64 = percent.strip_suffix('%')?.parse().ok()?;
    Some((value / 100.0).clamp(0.0, 1.0))
}

/// `http.client.responses`, for the codes a download meets.
fn http_reason(code: u16) -> &'static str {
    match code {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        408 => "Request Timeout",
        410 => "Gone",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "",
    }
}

/// The original `_run_cmd`: a shell command with its output (both streams)
/// read and dropped; a failure is `CalledProcessError`'s message. `lock` is
/// the Center to lock while a command through the sudo wrapper runs.
fn run_cmd(command: &str, lock: Option<Host>) -> Result<(), String> {
    let locking = lock.filter(|host| {
        let wrapper = host.sudo_wrapper_path();
        !wrapper.is_empty() && command.contains(&wrapper)
    });
    if let Some(host) = locking {
        host.lock_ui();
    }
    let status = shell_drained(command);
    if let Some(host) = locking {
        host.unlock_ui();
    }
    let status = status?;
    if status.success() {
        return Ok(());
    }
    Err(called_process_error(command, returncode(status)))
}

fn shell_drained(command: &str) -> Result<ExitStatus, String> {
    let (mut reader, writer) = std::io::pipe().map_err(|error| error.to_string())?;
    let error_writer = writer.try_clone().map_err(|error| error.to_string())?;
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .stdout(writer)
        .stderr(error_writer)
        .spawn()
        .map_err(|error| os_error(&error, &[Path::new("/bin/sh")]))?;
    // The writers went to the child; the read ends when it and whatever it
    // started are done with them.
    let mut sink = [0u8; 8192];
    while matches!(reader.read(&mut sink), Ok(read) if read > 0) {}
    child.wait().map_err(|error| error.to_string())
}

/// The original's `Popen(config_script, shell=True, stdin=PIPE)` answered
/// with `Y`: its return code (negative for a signal).
fn answer_yes(script: &Path) -> Result<i32, String> {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| os_error(&error, &[Path::new("/bin/sh")]))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A script that never reads its answer is fine (communicate ignores
        // the broken pipe too).
        let _ = stdin.write_all(b"Y\n");
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    Ok(returncode(status))
}

/// `Popen.returncode`: the exit status, or minus the signal.
fn returncode(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or(0))
}

/// `str(subprocess.CalledProcessError(returncode, command))`.
fn called_process_error(command: &str, returncode: i32) -> String {
    if returncode < 0 {
        match signal_name(-returncode) {
            Some(name) => format!(
                "Command '{command}' died with <Signals.{name}: {}>.",
                -returncode
            ),
            None => format!(
                "Command '{command}' died with unknown signal {}.",
                -returncode
            ),
        }
    } else {
        format!("Command '{command}' returned non-zero exit status {returncode}.")
    }
}

fn signal_name(signal: i32) -> Option<&'static str> {
    Some(match signal {
        libc::SIGHUP => "SIGHUP",
        libc::SIGINT => "SIGINT",
        libc::SIGQUIT => "SIGQUIT",
        libc::SIGILL => "SIGILL",
        libc::SIGTRAP => "SIGTRAP",
        libc::SIGABRT => "SIGABRT",
        libc::SIGBUS => "SIGBUS",
        libc::SIGFPE => "SIGFPE",
        libc::SIGKILL => "SIGKILL",
        libc::SIGUSR1 => "SIGUSR1",
        libc::SIGSEGV => "SIGSEGV",
        libc::SIGUSR2 => "SIGUSR2",
        libc::SIGPIPE => "SIGPIPE",
        libc::SIGALRM => "SIGALRM",
        libc::SIGTERM => "SIGTERM",
        libc::SIGCHLD => "SIGCHLD",
        libc::SIGCONT => "SIGCONT",
        libc::SIGSTOP => "SIGSTOP",
        libc::SIGTSTP => "SIGTSTP",
        libc::SIGTTIN => "SIGTTIN",
        libc::SIGTTOU => "SIGTTOU",
        libc::SIGXCPU => "SIGXCPU",
        libc::SIGXFSZ => "SIGXFSZ",
        libc::SIGSYS => "SIGSYS",
        _ => return None,
    })
}

/// `shlex.quote`.
pub fn shell_quote(text: &[u8]) -> String {
    let text = String::from_utf8_lossy(text);
    if text.is_empty() {
        return "''".into();
    }
    let safe = |c: char| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c);
    if text.chars().all(safe) {
        return text.into_owned();
    }
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

/// `shutil.move` of a file: a rename, else a copy and an unlink.
fn move_file(source: &Path, target: &Path) -> Result<(), String> {
    if std::fs::rename(source, target).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, target).map_err(|error| os_error(&error, &[target]))?;
    std::fs::remove_file(source).map_err(|error| os_error(&error, &[source]))
}

/// `str(OSError)`: `[Errno 13] Permission denied: '/path'`.
fn os_error(error: &std::io::Error, paths: &[&Path]) -> String {
    let Some(code) = error.raw_os_error() else {
        return error.to_string();
    };
    // SAFETY: strerror returns a NUL-terminated message for any number.
    let message = unsafe { std::ffi::CStr::from_ptr(libc::strerror(code)) }.to_string_lossy();
    match paths {
        [] => format!("[Errno {code}] {message}"),
        [path] => format!("[Errno {code}] {message}: {}", python_repr(path)),
        [first, second, ..] => format!(
            "[Errno {code}] {message}: {} -> {}",
            python_repr(first),
            python_repr(second)
        ),
    }
}

/// `repr()` of a path as Python shows it in an error.
fn python_repr(path: &Path) -> String {
    let text = path.to_string_lossy();
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut shown = String::new();
    shown.push(quote);
    for c in text.chars() {
        match c {
            '\\' => shown.push_str("\\\\"),
            '\n' => shown.push_str("\\n"),
            '\r' => shown.push_str("\\r"),
            '\t' => shown.push_str("\\t"),
            c if c == quote => {
                shown.push('\\');
                shown.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                shown.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => shown.push(c),
        }
    }
    shown.push(quote);
    shown
}

/// The original `_launch_app`'s process: the app under `rum`, in a session of
/// its own. The caller waits for it.
pub fn launch(app: &AppInfo) -> std::io::Result<Child> {
    let exe = affinity_dir().join(app.folder).join(app.exe);
    let mut command = Command::new("rum");
    command
        .arg("wine-affinity")
        .arg(wine_prefix())
        .arg("wine")
        .arg(exe);
    // SAFETY: setsid is async-signal-safe (start_new_session=True).
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    command.spawn()
}

/// The original `_perform_removal`'s work: the menu entries, then the whole
/// prefix, ignoring what cannot be removed. Blocking.
pub fn remove() {
    let applications = applications_dir();
    for app in &APP_REGISTRY {
        let entry = applications.join(app.desktop);
        if entry.exists() {
            let _ = std::fs::remove_file(entry);
        }
    }
    let prefix = wine_prefix();
    if prefix.is_dir() {
        remove_tree(&prefix);
    }
}

/// `shutil.rmtree(path, ignore_errors=True)`: removes what it can and never
/// follows a symbolic link (a Wine prefix links its drives to `/` and home).
fn remove_tree(path: &Path) {
    match std::fs::symlink_metadata(path) {
        // rmtree refuses a link to a directory as the root.
        Ok(metadata) if metadata.file_type().is_dir() => {}
        _ => return,
    }
    remove_contents(path);
    let _ = std::fs::remove_dir(path);
}

fn remove_contents(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => {
                remove_contents(&path);
                let _ = std::fs::remove_dir(&path);
            }
            _ => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_matches_shlex() {
        assert_eq!(shell_quote(b""), "''");
        assert_eq!(
            shell_quote(b"/home/me/.WineAffinity"),
            "/home/me/.WineAffinity"
        );
        assert_eq!(
            shell_quote(b"/tmp/Affinity x64.exe"),
            "'/tmp/Affinity x64.exe'"
        );
        assert_eq!(shell_quote(b"it's"), "'it'\"'\"'s'");
        assert_eq!(shell_quote(b"$HOME"), "'$HOME'");
    }

    #[test]
    fn failures_read_like_python() {
        assert_eq!(
            called_process_error("false", 1),
            "Command 'false' returned non-zero exit status 1."
        );
        assert_eq!(
            called_process_error("sleep 9", -9),
            "Command 'sleep 9' died with <Signals.SIGKILL: 9>."
        );
        assert_eq!(
            called_process_error("x", -64),
            "Command 'x' died with unknown signal 64."
        );
        let error = std::io::Error::from_raw_os_error(libc::EACCES);
        assert_eq!(
            os_error(&error, &[Path::new("/root/x")]),
            "[Errno 13] Permission denied: '/root/x'"
        );
        assert_eq!(python_repr(Path::new("it's")), "\"it's\"");
        assert_eq!(python_repr(Path::new("a\\b")), "'a\\\\b'");
    }

    #[test]
    fn commands_report_their_status() {
        assert_eq!(run_cmd("echo out; echo err >&2", None), Ok(()));
        assert_eq!(
            run_cmd("exit 3", None),
            Err("Command 'exit 3' returned non-zero exit status 3.".into())
        );
        assert_eq!(answer_yes(Path::new("read a; [ \"$a\" = Y ]")), Ok(0));
        assert_eq!(answer_yes(Path::new("exit 4")), Ok(4));
        assert_eq!(answer_yes(Path::new("kill -TERM $$")), Ok(-15));
    }

    #[test]
    fn the_progress_bar_is_read() {
        let mut meter = Meter::default();
        let mut seen = meter.feed(b"\r##########            ");
        seen.extend(meter.feed(b"  42.5%\r#####################   100.0%\n"));
        seen.extend(
            meter.feed(b"\r#=#=#  \r-=O=-   \ncurl: (6) Could not resolve host: example\n"),
        );
        meter.finish();
        assert_eq!(seen, vec![0.425, 1.0]);
        assert_eq!(
            meter.said,
            vec!["curl: (6) Could not resolve host: example"]
        );
        assert_eq!(bar_fraction("0.0%"), Some(0.0));
        assert_eq!(bar_fraction("Warning: 50%"), None);
    }

    #[test]
    fn removal_never_follows_links() {
        let base = std::env::temp_dir().join(format!("affinity-remove-{}", std::process::id()));
        let keep = base.join("keep");
        let tree = base.join("tree");
        std::fs::create_dir_all(&keep).unwrap();
        std::fs::write(keep.join("precious"), b"x").unwrap();
        std::fs::create_dir_all(tree.join("drive_c/Program Files")).unwrap();
        std::fs::write(tree.join("drive_c/Program Files/app.exe"), b"x").unwrap();
        std::os::unix::fs::symlink(&keep, tree.join("z:")).unwrap();
        remove_tree(&tree);
        assert!(!tree.exists());
        assert!(keep.join("precious").exists());
        // A link as the root is left alone, as rmtree refuses it.
        std::os::unix::fs::symlink(&keep, base.join("link")).unwrap();
        remove_tree(&base.join("link"));
        assert!(keep.join("precious").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn the_registry_keeps_its_order() {
        let names: Vec<_> = APP_REGISTRY.iter().map(|app| app.name).collect();
        assert_eq!(names, ["Affinity", "Photo 2", "Designer 2", "Publisher 2"]);
        assert_eq!(total_steps(true), 6);
        assert_eq!(total_steps(false), 5);
    }
}
