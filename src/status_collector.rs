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
