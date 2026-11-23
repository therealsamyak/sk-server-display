//! Status collector: one source of readings for one status feed.

use crate::{
    collector_glances::GlancesCollector, collector_kuma::KumaCollector,
    collector_local::LocalCollector, collector_scrutiny::ScrutinyCollector,
    collector_tailscale::TailscaleCollector, status_model::FeedData,
};

/// Status collector: Glances, this machine, Uptime Kuma, Scrutiny, or Tailscale.
#[derive(Clone, Debug)]
pub enum StatusCollector {
    Glances(GlancesCollector),
    Local(LocalCollector),
    Kuma(KumaCollector),
    Scrutiny(ScrutinyCollector),
    Tailscale(TailscaleCollector),
}

impl StatusCollector {
    /// Reads one reading. This call blocks; run it on a background thread.
    pub fn collect_status_reading(&self) -> anyhow::Result<FeedData> {
        Ok(match self {
            Self::Glances(collector) => FeedData::Host(collector.fetch_glances_metrics()?),
            Self::Local(collector) => FeedData::Host(collector.fetch_local_metrics()?),
            Self::Kuma(collector) => FeedData::Services(collector.fetch_kuma_services()?),
            Self::Scrutiny(collector) => FeedData::Disks(collector.fetch_scrutiny_disks()?),
            Self::Tailscale(collector) => FeedData::Tailnet(collector.fetch_tailnet_devices()?),
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
