//! Tailscale collectors: tailnet devices from `tailscale status --json`.
//!
//! The collector only reads status. It runs the local Tailscale command
//! line tool and changes nothing.

use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::status_model::{TailnetConnection, TailnetDevice};

/// Time that one `tailscale status --json` result stays valid for other
/// feeds. The tailnet feed and the host cards poll at the same time, so they
/// share one command run instead of one run each.
const TAILSCALE_STATUS_REUSE: Duration = Duration::from_secs(2);

/// Age limit of a Tailscale result for the presence of a Glances host that
/// does not answer. It is the tailnet poll interval, so a Glances host that
/// is asleep adds no command runs.
const TAILSCALE_PRESENCE_MAX_AGE: Duration = Duration::from_secs(10);

/// Last `tailscale status --json` result: the time of the run and the devices
/// or the error text.
type TailscaleStatusResult = (Instant, Result<Arc<Vec<TailnetDevice>>, String>);

/// Shared reader of `tailscale status --json` for all Tailscale feeds.
#[derive(Clone, Debug)]
pub struct TailscaleStatusReader {
    /// Command name or path, such as `tailscale`.
    command: String,
    last: Arc<Mutex<Option<TailscaleStatusResult>>>,
}

impl TailscaleStatusReader {
    /// Creates a reader that runs `command status --json`.
    pub fn new_tailscale_reader(command: String) -> Self {
        Self {
            command,
            last: Arc::default(),
        }
    }

    /// Test reader with a fresh result that holds these devices.
    #[cfg(test)]
    pub fn tailscale_reader_with_devices(devices: Vec<TailnetDevice>) -> Self {
        let reader = Self::new_tailscale_reader("tailscale".into());
        *reader.last.lock().unwrap() = Some((Instant::now(), Ok(Arc::new(devices))));
        reader
    }

    /// Reads the device with this name for a host card. A result from the
    /// last 10 seconds is used again. This call blocks; run it on a
    /// background thread.
    pub fn read_host_presence(&self, name: &str) -> anyhow::Result<TailnetDevice> {
        let devices = self.read_tailnet_devices(TAILSCALE_PRESENCE_MAX_AGE)?;
        find_tailnet_device(&devices, name)
    }

    /// Reads tailnet devices. A result newer than `max_age` is used again.
    /// This call blocks; run it on a background thread.
    fn read_tailnet_devices(&self, max_age: Duration) -> anyhow::Result<Arc<Vec<TailnetDevice>>> {
        // The lock stays held while the command runs, so feeds that poll at
        // the same time wait for one run and then share its result.
        let mut last = self
            .last
            .lock()
            .map_err(|_| anyhow::anyhow!("Tailscale status lock is poisoned"))?;
        if let Some((read_at, result)) = last.as_ref()
            && read_at.elapsed() < max_age
        {
            return result.clone().map_err(anyhow::Error::msg);
        }
        let result = read_tailscale_status(&self.command)
            .map(|status| Arc::new(tailnet_devices(status)))
            .map_err(|error| format!("{error:#}"));
        *last = Some((Instant::now(), result.clone()));
        result.map_err(anyhow::Error::msg)
    }
}

/// Tailscale collector: all tailnet devices.
#[derive(Clone, Debug)]
pub struct TailscaleCollector {
    pub reader: TailscaleStatusReader,
}

#[derive(Deserialize)]
pub(crate) struct TailscaleStatus {
    #[serde(rename = "Self")]
    pub this_machine: TailscalePeer,
    #[serde(rename = "Peer", default)]
    pub peers: HashMap<String, TailscalePeer>,
}

#[derive(Deserialize)]
pub(crate) struct TailscalePeer {
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "DNSName", default)]
    pub dns_name: String,
    #[serde(rename = "OS", default)]
    pub os: String,
    #[serde(rename = "Online", default)]
    pub online: bool,
    #[serde(rename = "Active", default)]
    pub active: bool,
    #[serde(rename = "CurAddr", default)]
    pub current_address: String,
    #[serde(rename = "Relay", default)]
    pub relay: String,
    #[serde(rename = "LastSeen", default, with = "time::serde::rfc3339::option")]
    pub last_seen: Option<OffsetDateTime>,
}

impl TailscaleCollector {
    /// Reads tailnet devices. This call blocks; run it on a background thread.
    pub fn fetch_tailnet_devices(&self) -> anyhow::Result<Vec<TailnetDevice>> {
        Ok(self
            .reader
            .read_tailnet_devices(TAILSCALE_STATUS_REUSE)?
            .as_ref()
            .clone())
    }
}

/// Tailnet host collector: one device from `tailscale status --json`, for a
/// host card without a metrics server.
#[derive(Clone, Debug)]
pub struct TailnetHostCollector {
    pub reader: TailscaleStatusReader,
    /// Device name: the first label of its MagicDNS name, such as `server`.
    pub name: String,
}

impl TailnetHostCollector {
    /// Reads the device. This call blocks; run it on a background thread.
    pub fn fetch_tailnet_host(&self) -> anyhow::Result<TailnetDevice> {
        let devices = self.reader.read_tailnet_devices(TAILSCALE_STATUS_REUSE)?;
        find_tailnet_device(&devices, &self.name)
    }
}

/// Finds one device by name. Case does not matter.
pub(crate) fn find_tailnet_device(
    devices: &[TailnetDevice],
    name: &str,
) -> anyhow::Result<TailnetDevice> {
    devices
        .iter()
        .find(|device| device.name.eq_ignore_ascii_case(name))
        .cloned()
        .with_context(|| format!("{name} is not in the tailnet"))
}

/// Runs `tailscale status --json` and parses its output.
fn read_tailscale_status(command_name: &str) -> anyhow::Result<TailscaleStatus> {
    let mut command = Command::new(command_name);
    command.args(["status", "--json"]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        // CREATE_NO_WINDOW: do not flash a console window on each poll.
        command.creation_flags(0x0800_0000);
    }
    let output = command
        .output()
        .with_context(|| format!("cannot run `{command_name} status --json`"))?;
    if !output.status.success() {
        bail!(
            "`{command_name} status --json` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("Tailscale status is not valid JSON")
}

/// Converts Tailscale status to devices: this machine first, then online
/// devices, then offline devices, each group by name.
pub(crate) fn tailnet_devices(status: TailscaleStatus) -> Vec<TailnetDevice> {
    let device = |peer: TailscalePeer, is_self: bool| {
        let name = peer
            .dns_name
            .split('.')
            .next()
            .filter(|label| !label.is_empty())
            .unwrap_or(&peer.host_name)
            .to_string();
        let connection = if is_self {
            TailnetConnection::ThisMachine
        } else if !peer.online {
            TailnetConnection::Offline
        } else if !peer.current_address.is_empty() {
            TailnetConnection::Direct
        } else if peer.active && !peer.relay.is_empty() {
            TailnetConnection::Relay(peer.relay.clone())
        } else {
            TailnetConnection::Idle
        };
        TailnetDevice {
            name,
            os: peer.os,
            online: is_self || peer.online,
            is_self,
            // Tailscale reports year 1 for devices that are online now.
            last_seen: peer.last_seen.filter(|at| at.year() > 1),
            connection,
        }
    };
    let mut devices: Vec<TailnetDevice> = status
        .peers
        .into_values()
        .map(|peer| device(peer, false))
        .collect();
    devices.sort_by(|a, b| b.online.cmp(&a.online).then(a.name.cmp(&b.name)));
    devices.insert(0, device(status.this_machine, true));
    devices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_devices_and_classifies_connections() {
        let status: TailscaleStatus = serde_json::from_str(
            r#"{
              "Self": {"HostName": "Laptop", "DNSName": "laptop.example.ts.net.", "OS": "windows", "Online": true},
              "Peer": {
                "a": {"HostName": "Old Tablet", "DNSName": "old-tablet.example.ts.net.", "OS": "android",
                      "Online": false, "LastSeen": "2026-11-21T16:11:41.1Z"},
                "b": {"HostName": "Server", "DNSName": "server.example.ts.net.", "OS": "linux",
                      "Online": true, "Active": true, "CurAddr": "192.0.2.10:41641", "Relay": "lax",
                      "LastSeen": "0001-01-01T00:00:00Z"},
                "c": {"HostName": "Phone", "DNSName": "phone.example.ts.net.", "OS": "iOS",
                      "Online": true, "Active": true, "CurAddr": "", "Relay": "lax"},
                "d": {"HostName": "Desk", "DNSName": "", "OS": "macOS", "Online": true}
              }
            }"#,
        )
        .unwrap();
        let devices = tailnet_devices(status);
        let names: Vec<_> = devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["laptop", "Desk", "phone", "server", "old-tablet"]);
        assert_eq!(devices[0].connection, TailnetConnection::ThisMachine);
        assert_eq!(devices[1].connection, TailnetConnection::Idle);
        assert_eq!(
            devices[2].connection,
            TailnetConnection::Relay("lax".into())
        );
        assert_eq!(devices[3].connection, TailnetConnection::Direct);
        assert_eq!(devices[3].last_seen, None);
        assert_eq!(devices[4].connection, TailnetConnection::Offline);
        assert!(devices[4].last_seen.is_some());
        let found = find_tailnet_device(&devices, "SERVER").unwrap();
        assert_eq!(found.name, "server");
        assert!(find_tailnet_device(&devices, "missing").is_err());
    }

    #[test]
    fn tailscale_reader_shares_one_result() {
        // A missing command fails. The second read gets the same error from
        // the shared result instead of a second run.
        let reader = TailscaleStatusReader::new_tailscale_reader(
            "server-status-display-missing-tailscale".into(),
        );
        let first = reader
            .read_tailnet_devices(TAILSCALE_STATUS_REUSE)
            .unwrap_err()
            .to_string();
        let read_at = reader.last.lock().unwrap().as_ref().map(|(at, _)| *at);
        let second = reader
            .read_tailnet_devices(TAILSCALE_STATUS_REUSE)
            .unwrap_err()
            .to_string();
        assert_eq!(first, second);
        assert_eq!(
            reader.last.lock().unwrap().as_ref().map(|(at, _)| *at),
            read_at
        );
    }
}
