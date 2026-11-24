//! Tailscale collector: tailnet devices from `tailscale status --json`.
//!
//! The collector only reads status. It runs the local Tailscale command
//! line tool and changes nothing.

use std::{collections::HashMap, process::Command};

use anyhow::{Context as _, bail};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::status_model::{TailnetConnection, TailnetDevice};

/// Tailscale collector that runs the local `tailscale` command.
#[derive(Clone, Debug)]
pub struct TailscaleCollector {
    /// Command name or path, such as `tailscale`.
    pub command: String,
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
        let mut command = Command::new(&self.command);
        command.args(["status", "--json"]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt as _;
            // CREATE_NO_WINDOW: do not flash a console window on each poll.
            command.creation_flags(0x0800_0000);
        }
        let output = command
            .output()
            .with_context(|| format!("cannot run `{} status --json`", self.command))?;
        if !output.status.success() {
            bail!(
                "`{} status --json` failed: {}",
                self.command,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let status: TailscaleStatus =
            serde_json::from_slice(&output.stdout).context("Tailscale status is not valid JSON")?;
        Ok(tailnet_devices(status))
    }
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
    }
}
