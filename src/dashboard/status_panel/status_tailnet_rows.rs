use crate::metrics::{StatusCollector, StatusFeed, TailnetDevice};

/// Tailnet devices already represented by configured host cards.
#[derive(Clone, Default)]
pub(in crate::dashboard) struct CardedTailnetDevices {
    has_local_card: bool,
    named_devices: Vec<String>,
}

impl CardedTailnetDevices {
    /// Identifies devices with host cards, regardless of feed freshness.
    pub(in crate::dashboard) fn from_status_feeds(feeds: &[StatusFeed]) -> Self {
        Self {
            has_local_card: feeds
                .iter()
                .any(|feed| matches!(feed.spec.collector, StatusCollector::Local(_))),
            named_devices: feeds
                .iter()
                .filter_map(|feed| feed.spec.collector.tailnet_device_name().map(str::to_owned))
                .collect(),
        }
    }

    /// Lists the devices shown in the Tailnet panel.
    pub(super) fn visible_devices<'a>(
        &'a self,
        devices: &'a [TailnetDevice],
    ) -> impl Iterator<Item = &'a TailnetDevice> {
        devices.iter().filter(|device| {
            if device.is_self {
                return !self.has_local_card;
            }
            !self
                .named_devices
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&device.name))
        })
    }
}
