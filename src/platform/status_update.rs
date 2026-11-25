//! Verify a release binary and replace a portable executable after it exits.

use std::{
    env,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write as _},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};
use sha2::{Digest as _, Sha256};

use super::status_release::ReleaseUpdate;

const WORKER_ARG: &str = "--status-update-worker";
const STAGE_PREFIX: &str = ".server-status-update-";
const STAGED_BINARY: &str = "pending-build";
#[cfg(windows)]
const WORKER_BINARY: &str = "update-worker.exe";
#[cfg(not(windows))]
const WORKER_BINARY: &str = "update-worker";
const BACKUP_BINARY: &str = "previous-build";
const READY_MARKER: &str = "first-frame";
const UPDATE_ERROR: &str = "STATUS_DISPLAY_UPDATE_ERROR";
const CLEANUP_DIR: &str = "STATUS_DISPLAY_UPDATE_CLEANUP";

/// A verified build and a worker staged beside the installed executable.
pub(crate) struct PreparedAppUpdate {
    folder: tempfile::TempDir,
    executable: PathBuf,
}

/// Download and verify the release without changing the installed binary.
pub(crate) fn prepare_app_update(update: &ReleaseUpdate) -> anyhow::Result<PreparedAppUpdate> {
    let executable = env::current_exe()
        .context("Could not locate the installed status display.")?
        .canonicalize()?;
    let parent = executable
        .parent()
        .context("The app needs an install folder.")?;
    let folder = tempfile::Builder::new()
        .prefix(STAGE_PREFIX)
        .tempdir_in(parent)
        .context("The app's install folder must be writable.")?;
    let staged = folder.path().join(STAGED_BINARY);
    let response = update.download_release_asset()?;
    copy_verified_build(
        response.into_body().into_reader(),
        &staged,
        &update.sha256,
        update.size,
    )?;
    fs::set_permissions(&staged, fs::metadata(&executable)?.permissions())?;

    let worker = folder.path().join(WORKER_BINARY);
    fs::copy(&executable, &worker).context("Could not prepare the update worker.")?;
    Ok(PreparedAppUpdate { folder, executable })
}

impl PreparedAppUpdate {
    /// Start the worker and return the signal to hold until application shutdown.
    pub(crate) fn start_update_worker(self) -> anyhow::Result<ChildStdin> {
        let worker = self.folder.path().join(WORKER_BINARY);
        let mut command = Command::new(worker);
        command
            .arg(WORKER_ARG)
            .arg(&self.executable)
            .args(env::args_os().skip(1))
            .stdin(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .context("Could not start the update worker.")?;
        let signal = child
            .stdin
            .take()
            .context("Could not open the update worker's exit signal.")?;
        let _ = self.folder.keep();
        Ok(signal)
    }
}

fn copy_verified_build(
    source: impl Read,
    destination: &Path,
    expected: &str,
    expected_size: u64,
) -> anyhow::Result<()> {
    let mut source = source.take(expected_size + 1);
    let mut header = [0; 4];
    source
        .read_exact(&mut header)
        .context("Could not read the update executable.")?;
    #[cfg(target_os = "windows")]
    let executable = header.starts_with(b"MZ");
    #[cfg(target_os = "linux")]
    let executable = header == *b"\x7fELF";
    #[cfg(target_os = "macos")]
    let executable = matches!(
        u32::from_be_bytes(header),
        0xfeedfacf | 0xcffaedfe | 0xcafebabe | 0xcafebabf
    );
    ensure!(
        executable,
        "Select a status display executable for this operating system."
    );
    let mut file = File::create_new(destination)?;
    let mut sha256 = Sha256::new();
    sha256.update(header);
    file.write_all(&header)?;
    let mut size = header.len() as u64;
    let mut buffer = vec![0; 128 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let bytes = &buffer[..count];
        sha256.update(bytes);
        size += count as u64;
        file.write_all(bytes)?;
    }
    ensure!(
        size == expected_size,
        "The download does not match the release file size."
    );
    let actual = format!("{:x}", sha256.finalize());
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "The download does not match the release SHA-256 checksum."
    );
    file.sync_all()?;
    Ok(())
}

/// Run the staged update worker before starting GPUI.
pub(crate) fn handle_app_update_worker() -> Option<anyhow::Result<()>> {
    let mut args = env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(WORKER_ARG)) {
        return None;
    }
    Some((|| {
        let target = PathBuf::from(
            args.next()
                .context("Update worker needs an install path.")?,
        );
        let worker = env::current_exe()?.canonicalize()?;
        let args: Vec<_> = args.collect();
        let staging = worker
            .parent()
            .context("Update worker needs a staging folder.")?;
        ensure!(
            is_update_staging(staging, &target)
                && worker.file_name() == Some(OsStr::new(WORKER_BINARY)),
            "Invalid update staging folder."
        );
        // The GPUI process owns the writing end until shutdown.
        io::copy(&mut io::stdin().lock(), &mut io::sink())?;
        let launch = |error: Option<&str>| -> io::Result<Child> {
            let mut command = Command::new(&target);
            command.args(&args);
            command.env_remove(UPDATE_ERROR).env_remove(CLEANUP_DIR);
            if let Some(error) = error {
                command.env(UPDATE_ERROR, error);
            }
            command.env(CLEANUP_DIR, staging);
            command.spawn()
        };
        let _install_lock = match lock_installation(&target) {
            Ok(lock) => lock,
            Err(error) => {
                let message = format!("Could not start the update. {error:#}");
                launch(Some(&message)).context(message)?;
                return Ok(());
            }
        };
        let backup = staging.join(BACKUP_BINARY);
        let install = fs::copy(&target, &backup)
            .and_then(|_| replace_executable(&staging.join(STAGED_BINARY), &target));
        if let Err(error) = install {
            let message = format!("Could not install the update. {error}");
            launch(Some(&message)).context(message)?;
        } else if let Err(error) = launch(None)
            .map_err(anyhow::Error::from)
            .and_then(|child| wait_for_update_window(child, &staging.join(READY_MARKER)))
        {
            let message =
                format!("The update could not start. The previous build was restored. {error}");
            replace_executable(&backup, &target).with_context(|| {
                format!(
                    "Could not restore the previous build at {}",
                    backup.display()
                )
            })?;
            launch(Some(&message)).context(message)?;
        }
        Ok(())
    })())
}

fn wait_for_update_window(mut child: Child, ready: &Path) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if ready.is_file() {
            if let Err(error) = fs::remove_file(ready) {
                eprintln!("Could not acknowledge the updated window: {error}");
            }
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("The updated build exited before its first window ({status}).");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait()?;
            anyhow::bail!("The updated build did not open a window within 30 seconds.");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn lock_installation(target: &Path) -> anyhow::Result<File> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(target.with_file_name(".server-status-update.lock"))?;
    lock.try_lock()
        .context("Another status display update is in progress.")?;
    Ok(lock)
}

fn replace_executable(from: &Path, target: &Path) -> io::Result<()> {
    #[cfg(windows)]
    for _ in 0..40 {
        match fs::rename(from, target) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error),
        }
    }
    fs::rename(from, target)
}

fn is_update_staging(folder: &Path, target: &Path) -> bool {
    folder.is_absolute()
        && folder.parent() == target.parent()
        && folder
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| {
                name.strip_prefix(STAGE_PREFIX).is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
                })
            })
}

/// Acknowledge the restarted window and return any update failure to show.
pub(crate) fn finish_app_update() -> Option<String> {
    use std::sync::atomic::{AtomicBool, Ordering};

    static FINISHED: AtomicBool = AtomicBool::new(false);
    if FINISHED.swap(true, Ordering::Relaxed) {
        return None;
    }
    let folder = PathBuf::from(env::var_os(CLEANUP_DIR)?);
    let executable = env::current_exe()
        .and_then(|path| path.canonicalize())
        .ok()?;
    if !is_update_staging(&folder, &executable) {
        return None;
    }
    let error = env::var(UPDATE_ERROR).ok();
    let ready = folder.join(READY_MARKER);
    let updated = error.is_none();
    if updated && let Err(error) = fs::write(&ready, []) {
        log::warn!("Could not signal the update worker: {error}");
        return None;
    }
    thread::spawn(move || {
        for _ in 0..30 {
            if updated && ready.exists() {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            for file in [STAGED_BINARY, BACKUP_BINARY, WORKER_BINARY] {
                let _ = fs::remove_file(folder.join(file));
            }
            match fs::remove_dir(&folder) {
                Ok(()) => return,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return,
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        }
        log::warn!("Could not remove the temporary update folder.");
    });
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_exact_update_bytes_before_use() {
        let directory = tempfile::tempdir().unwrap();
        #[cfg(target_os = "windows")]
        let content = b"MZ\0\0test update bytes";
        #[cfg(target_os = "linux")]
        let content = b"\x7fELFtest update bytes";
        #[cfg(target_os = "macos")]
        let content = b"\xfe\xed\xfa\xcftest update bytes";
        let destination = directory.path().join("verified");
        let sha256 = format!("{:x}", Sha256::digest(content));
        copy_verified_build(
            content.as_slice(),
            &destination,
            &sha256,
            content.len() as u64,
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), content);
        let incorrect = directory.path().join("incorrect");
        assert!(
            copy_verified_build(
                content.as_slice(),
                &incorrect,
                &"0".repeat(64),
                content.len() as u64
            )
            .is_err()
        );
    }

    #[test]
    fn scopes_cleanup_to_an_update_folder_beside_the_executable() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("app");
        assert!(is_update_staging(
            &directory.path().join(".server-status-update-test123"),
            &executable
        ));
        assert!(!is_update_staging(
            &directory.path().join("profile"),
            &executable
        ));
        assert!(!is_update_staging(
            &directory.path().join("profile/.server-status-update-test"),
            &executable
        ));
    }
}
