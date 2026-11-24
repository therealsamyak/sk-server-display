//! Status collector: one source of readings for one status feed.

use crate::{
    collector_glances::GlancesCollector,
    collector_kuma::KumaCollector,
    collector_local::LocalCollector,
    collector_scrutiny::ScrutinyCollector,
    collector_tailscale::{TailnetHostCollector, TailscaleCollector},
    status_model::{FeedData, HostPresence},
};

/// Status collector: Glances, this machine, Uptime Kuma, Scrutiny, the
/// tailnet, or one tailnet host.
#[derive(Clone, Debug)]
pub enum StatusCollector {
    Glances(GlancesCollector),
    Local(LocalCollector),
    Kuma(KumaCollector),
    Scrutiny(ScrutinyCollector),
    Tailscale(TailscaleCollector),
    TailnetHost(TailnetHostCollector),
}

impl StatusCollector {
    /// `true` for collectors that fill a host card in the host column.
    pub fn is_host_collector(&self) -> bool {
        matches!(
            self,
            Self::Glances(_) | Self::Local(_) | Self::TailnetHost(_)
        )
    }

    /// Reads one reading. This call blocks; run it on a background thread.
    pub fn collect_status_reading(&self) -> anyhow::Result<FeedData> {
        Ok(match self {
            Self::Glances(collector) => collector.fetch_glances_reading()?,
            Self::Local(collector) => FeedData::Host(collector.fetch_local_metrics()?),
            Self::Kuma(collector) => FeedData::Services(collector.fetch_kuma_services()?),
            Self::Scrutiny(collector) => FeedData::Disks(collector.fetch_scrutiny_disks()?),
            Self::Tailscale(collector) => FeedData::Tailnet(collector.fetch_tailnet_devices()?),
            Self::TailnetHost(collector) => FeedData::HostPresence(HostPresence {
                device: collector.fetch_tailnet_host()?,
                metrics_error: None,
            }),
        })
    }
}

/// Returns `true` for loopback, VPN, container, and virtual machine network
/// interfaces. The status display shows the traffic of a physical interface.
pub fn is_virtual_network_interface(name: &str) -> bool {
    const VIRTUAL_PREFIXES: [&str; 5] = ["lo", "br-", "veth", "virbr", "vEthernet"];
    const VIRTUAL_PARTS: [&str; 6] = [
        "loopback",
        "tailscale",
        "docker",
        "virtualbox",
        "vmware",
        "wsl",
    ];
    let lower = name.to_lowercase();
    VIRTUAL_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || VIRTUAL_PARTS.iter().any(|part| lower.contains(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_virtual_network_interfaces() {
        for name in [
            "lo",
            "tailscale0",
            "docker0",
            "br-1a2b",
            "vethab12",
            "vEthernet (WSL)",
        ] {
            assert!(is_virtual_network_interface(name), "{name}");
        }
        for name in ["eth0", "enp5s0", "wlan0", "Ethernet", "Wi-Fi"] {
            assert!(!is_virtual_network_interface(name), "{name}");
        }
    }
}
