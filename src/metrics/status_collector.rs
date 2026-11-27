//! Status collector: one source of readings for one status feed.

use crate::{
    metrics::collectors::collector_glances::GlancesCollector,
    metrics::collectors::collector_kuma::KumaCollector,
    metrics::collectors::collector_local::LocalCollector,
    metrics::collectors::collector_scrutiny::ScrutinyCollector,
    metrics::collectors::collector_tailscale::{TailnetHostCollector, TailscaleCollector},
    metrics::status_model::{FeedData, HostPresence},
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

/// Feed expansion group: Devices or Services. Each group holds at most one
/// expanded card, independent of the other group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedExpansionGroup {
    Devices,
    Services,
}

impl StatusCollector {
    /// Reports whether the feed has a compact summary and expanded panel.
    pub fn has_expandable_panel(&self) -> bool {
        self.feed_expansion_group().is_some()
    }

    /// Expansion group of an expandable feed card. Other feeds give none.
    pub fn feed_expansion_group(&self) -> Option<FeedExpansionGroup> {
        match self {
            Self::Local(_) | Self::Glances(_) => Some(FeedExpansionGroup::Devices),
            Self::Kuma(_) => Some(FeedExpansionGroup::Services),
            Self::Scrutiny(_) | Self::Tailscale(_) | Self::TailnetHost(_) => None,
        }
    }

    /// Reports whether the feed offers a btop process view.
    pub fn shows_host_btop(&self) -> bool {
        matches!(self, Self::Glances(collector) if collector.show_btop)
    }

    /// `true` for collectors that fill a host card in the host column.
    pub fn is_host_collector(&self) -> bool {
        matches!(
            self,
            Self::Glances(_) | Self::Local(_) | Self::TailnetHost(_)
        )
    }

    /// Return the Tailscale device referenced by a host card, if configured.
    pub fn tailnet_device_name(&self) -> Option<&str> {
        match self {
            Self::Glances(glances) => glances.presence.as_ref().map(|host| host.name.as_str()),
            Self::TailnetHost(host) => Some(&host.name),
            Self::Local(_) | Self::Kuma(_) | Self::Scrutiny(_) | Self::Tailscale(_) => None,
        }
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
/// macOS names its tunnels `utun` (Tailscale too), and its Apple wireless
/// links `awdl` and `llw`.
pub fn is_virtual_network_interface(name: &str) -> bool {
    const VIRTUAL_PREFIXES: [&str; 11] = [
        "lo",
        "br-",
        "veth",
        "virbr",
        "vEthernet",
        "utun",
        "awdl",
        "llw",
        "anpi",
        "gif",
        "stf",
    ];
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
            "utun4",
            "awdl0",
            "llw0",
        ] {
            assert!(is_virtual_network_interface(name), "{name}");
        }
        for name in ["eth0", "enp5s0", "wlan0", "Ethernet", "Wi-Fi", "en0", "en1"] {
            assert!(!is_virtual_network_interface(name), "{name}");
        }
    }
}
