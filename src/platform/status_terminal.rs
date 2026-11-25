//! Launch the selected host's btop in a separate terminal window.

use std::process::{Command, Stdio};

use anyhow::{Context as _, ensure};

/// Terminal command for the local machine or a configured OpenSSH destination.
#[derive(Clone, Debug)]
pub enum StatusTerminal {
    #[cfg(not(target_os = "windows"))]
    Local,
    Ssh(String),
}

impl StatusTerminal {
    /// Return the local btop terminal action where btop is supported.
    pub fn local_btop() -> Option<Self> {
        #[cfg(not(target_os = "windows"))]
        {
            Some(Self::Local)
        }
        #[cfg(target_os = "windows")]
        {
            None
        }
    }

    /// Parse an OpenSSH destination without accepting command-line options.
    pub fn ssh_btop(destination: &str) -> anyhow::Result<Self> {
        ensure!(
            !destination.is_empty()
                && !destination.starts_with('-')
                && destination.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'@' | b'.' | b'_' | b'-')
                }),
            "btop_ssh must be an SSH alias or user@host"
        );
        Ok(Self::Ssh(destination.into()))
    }

    fn btop_command(&self) -> (&str, Vec<&str>) {
        match self {
            #[cfg(not(target_os = "windows"))]
            Self::Local => ("btop", Vec::new()),
            Self::Ssh(destination) => (
                "ssh",
                vec![
                    "-tt",
                    "-o",
                    "StrictHostKeyChecking=yes",
                    "-o",
                    "ConnectTimeout=10",
                    "--",
                    destination,
                    "btop",
                ],
            ),
        }
    }

    /// Open btop in a separate terminal. The terminal handles SSH prompts.
    pub fn launch_btop(&self) -> anyhow::Result<()> {
        let (program, args) = self.btop_command();
        let mut terminal = platform_terminal(program, &args);
        let mut child = terminal
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| {
                format!(
                    "Could not open {}",
                    terminal.get_program().to_string_lossy()
                )
            })?;
        std::thread::spawn(move || match child.wait() {
            Ok(status) if !status.success() => log::warn!("btop terminal exited: {status}"),
            Ok(_) => {}
            Err(error) => log::warn!("could not wait for btop terminal: {error}"),
        });
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn platform_terminal(program: &str, args: &[&str]) -> Command {
    let mut terminal = Command::new("konsole");
    terminal
        .args(["--separate", "--fullscreen", "-e", program])
        .args(args);
    terminal
}

#[cfg(target_os = "windows")]
fn platform_terminal(program: &str, args: &[&str]) -> Command {
    let mut terminal = Command::new("wt.exe");
    terminal.args(["-w", "new", "new-tab", program]).args(args);
    terminal
}

#[cfg(target_os = "macos")]
fn platform_terminal(program: &str, args: &[&str]) -> Command {
    const OPEN_TERMINAL: &str = r#"
on run arguments
    set commandLine to ""
    repeat with argument in arguments
        set commandLine to commandLine & " " & quoted form of (contents of argument)
    end repeat
    tell application "Terminal"
        activate
        do script commandLine
    end tell
end run
"#;
    let mut terminal = Command::new("osascript");
    terminal.args(["-e", OPEN_TERMINAL, program]).args(args);
    terminal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_btop_uses_a_fixed_command_and_checks_host_keys() {
        let target = StatusTerminal::ssh_btop("operator@server").expect("valid target");
        let (program, args) = target.btop_command();
        assert_eq!(program, "ssh");
        assert!(args.contains(&"StrictHostKeyChecking=yes"));
        assert!(args.ends_with(&["--", "operator@server", "btop"]));
    }

    #[test]
    fn ssh_target_rejects_options_and_shell_syntax() {
        for invalid in ["", "-oProxyCommand=ls", "host;ls", "host name"] {
            assert!(StatusTerminal::ssh_btop(invalid).is_err());
        }
    }
}
