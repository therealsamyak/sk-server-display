mod collectors;
mod status_board;
mod status_collector;
mod status_config;
mod status_feed;
mod status_freshness;
#[cfg(test)]
mod status_lan_tests;
mod status_model;

pub(crate) use status_board::{StatusBoard, StatusFeedUpdated};
pub(crate) use status_collector::StatusCollector;
#[cfg(test)]
pub(crate) use status_config::build_status_feeds;
pub(crate) use status_config::{
    StatusConfig, default_status_config_path, load_status_config, load_status_feed_config,
    status_state_path,
};
pub(crate) use status_feed::{HOST_HISTORY_SAMPLES, HostHistory, StatusFeed};
pub(crate) use status_freshness::FeedFreshness;
#[cfg(test)]
pub(crate) use status_model::ServiceHeartbeat;
pub(crate) use status_model::{
    DiskHealth, DiskState, FeedData, HostMetrics, HostPresence, ServiceHealth, ServiceState,
    TailnetDevice,
};
