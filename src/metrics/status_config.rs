//! Status display configuration: which hosts and services to read.
//!
//! The file is TOML. It stays on the display machine and never goes into
//! Git, because it names private hosts. See `config.example.toml`.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use ureq::{
    SendBody,
    http::{Request, Uri},
    middleware::MiddlewareNext,
};

use crate::{
    metrics::collectors::collector_glances::GlancesCollector,
    metrics::collectors::collector_kuma::KumaCollector,
    metrics::collectors::collector_local::LocalCollector,
    metrics::collectors::collector_scrutiny::ScrutinyCollector,
    metrics::collectors::collector_tailscale::{
        TailnetHostCollector, TailscaleCollector, TailscaleStatusReader,
    },
    metrics::status_collector::StatusCollector,
};

/// Poll interval of host metrics from this machine and from Glances.
pub const HOST_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Poll interval of Tailscale device status.
pub const TAILNET_POLL_INTERVAL: Duration = Duration::from_secs(10);
/// Poll interval of Uptime Kuma. Kuma heartbeats arrive about once a minute.
pub const SERVICE_POLL_INTERVAL: Duration = Duration::from_secs(30);
/// Poll interval of Scrutiny. Scrutiny collects SMART data about once an hour.
pub const DISK_POLL_INTERVAL: Duration = Duration::from_secs(300);

/// Status display configuration file contents.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StatusConfig {
    /// Folder with one wallpaper file for each status style, named after
    /// the style, such as `cyrene-bow.png`. JPEG and PNG files work. A
    /// relative path starts from the folder of the configuration file.
    /// If omitted, the preset gallery stays hidden.
    #[serde(default)]
    pub preset_wallpapers: Option<PathBuf>,
    /// Connect directly to metrics URLs with no proxy or redirects.
    #[serde(default)]
    pub direct_metrics_http: bool,
    /// Host cards, in column order. Empty: one card for this machine.
    #[serde(default, rename = "host")]
    pub hosts: Vec<HostConfig>,
    #[serde(default)]
    pub uptime_kuma: Option<KumaConfig>,
    #[serde(default)]
    pub scrutiny: Option<ScrutinyConfig>,
    #[serde(default)]
    pub tailscale: TailscaleConfig,
}

/// One named host card with an explicit data source.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
pub enum HostConfig {
    Local {
        name: String,
    },
    Glances {
        name: String,
        url: String,
        fallback_device: Option<String>,
        /// Show the in-app btop window for this host.
        #[serde(default)]
        show_btop: bool,
    },
    Tailscale {
        name: String,
        device: String,
    },
}

/// Uptime Kuma public status page.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KumaConfig {
    /// Uptime Kuma URL, such as `http://server.example.ts.net:3001`.
    pub url: String,
    /// Status page slug.
    pub status_page: String,
}

/// Scrutiny web server.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScrutinyConfig {
    /// Scrutiny URL, such as `http://server.example.ts.net:8080`.
    pub url: String,
}

/// Tailscale device list.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TailscaleConfig {
    #[serde(default)]
    pub show_devices: bool,
    /// Tailscale command name or path.
    #[serde(default = "default_tailscale_command")]
    pub command: String,
}

impl Default for TailscaleConfig {
    fn default() -> Self {
        Self {
            show_devices: false,
            command: default_tailscale_command(),
        }
    }
}

fn default_tailscale_command() -> String {
    "tailscale".into()
}

/// Default configuration file path, such as
/// `%APPDATA%\server-status-display\config.toml` on Windows.
pub fn default_status_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("server-status-display").join("config.toml"))
}

/// Slideshow state file: favorites, slideshows, and the style on screen.
/// The app writes it next to the configuration file, such as
/// `%APPDATA%\server-status-display\status-display-state.toml`.
pub fn status_state_path(explicit_config: Option<&Path>) -> Option<PathBuf> {
    let config = match explicit_config {
        Some(path) => path.to_path_buf(),
        None => default_status_config_path()?,
    };
    Some(config.with_file_name("status-display-state.toml"))
}

/// Reads the configuration. A missing default file gives the default
/// configuration: one local host card.
pub fn load_status_config(explicit: Option<&Path>) -> anyhow::Result<StatusConfig> {
    let path = match explicit {
        Some(path) => path.to_path_buf(),
        None => match default_status_config_path() {
            Some(path) if path.exists() => path,
            _ => return Ok(StatusConfig::default()),
        },
    };
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read status config {}", path.display()))?;
    let mut config: StatusConfig = toml::from_str(&text)
        .with_context(|| format!("status config {} is not valid", path.display()))?;
    if let (Some(wallpapers), Some(folder)) = (&config.preset_wallpapers, path.parent()) {
        config.preset_wallpapers = Some(folder.join(wallpapers));
    }
    Ok(config)
}

/// One status feed: a collector and the panel that shows its readings.
#[derive(Clone, Debug)]
pub struct StatusFeedSpec {
    /// Panel title, such as `server` or `Services`.
    pub title: String,
    /// Source name for the panel, such as `Glances`.
    pub source: &'static str,
    pub collector: StatusCollector,
    pub poll_interval: Duration,
    /// Whether the host card offers the in-app btop window.
    pub show_btop: bool,
}

/// Builds the status feeds in display order.
pub fn build_status_feeds(config: &StatusConfig) -> anyhow::Result<Vec<StatusFeedSpec>> {
    let agent = |interval: Duration| -> ureq::Agent {
        let builder = ureq::Agent::config_builder().timeout_global(Some(
            interval
                .mul_f32(0.75)
                .clamp(Duration::from_millis(1500), Duration::from_secs(10)),
        ));
        let builder = if config.direct_metrics_http {
            builder.proxy(None).max_redirects(0).middleware(
                |request: Request<SendBody>, next: MiddlewareNext| {
                    let response = next.handle(request)?;
                    if response.status().is_redirection() {
                        return Err(ureq::Error::StatusCode(response.status().as_u16()));
                    }
                    Ok(response)
                },
            )
        } else {
            builder
        };
        builder.build().into()
    };
    // One reader for all Tailscale feeds, so they share each command run.
    let tailscale = TailscaleStatusReader::new_tailscale_reader(config.tailscale.command.clone());
    let mut feeds = Vec::new();
    for host in &config.hosts {
        feeds.push(host_feed_spec(host, &tailscale, agent(HOST_POLL_INTERVAL))?);
    }
    if config.hosts.is_empty() {
        feeds.push(local_feed_spec("This machine".into()));
    }
    if let Some(kuma) = &config.uptime_kuma {
        feeds.push(StatusFeedSpec {
            title: "Services".into(),
            source: "Uptime Kuma",
            collector: StatusCollector::Kuma(KumaCollector {
                base_url: source_url(&kuma.url).context("invalid Uptime Kuma URL")?,
                status_page: kuma.status_page.clone(),
                agent: agent(SERVICE_POLL_INTERVAL),
            }),
            poll_interval: SERVICE_POLL_INTERVAL,
            show_btop: false,
        });
    }
    if let Some(scrutiny) = &config.scrutiny {
        feeds.push(StatusFeedSpec {
            title: "Disks".into(),
            source: "Scrutiny",
            collector: StatusCollector::Scrutiny(ScrutinyCollector {
                base_url: source_url(&scrutiny.url).context("invalid Scrutiny URL")?,
                agent: agent(DISK_POLL_INTERVAL),
            }),
            poll_interval: DISK_POLL_INTERVAL,
            show_btop: false,
        });
    }
    if config.tailscale.show_devices {
        feeds.push(StatusFeedSpec {
            title: "Tailnet".into(),
            source: "Tailscale",
            collector: StatusCollector::Tailscale(TailscaleCollector { reader: tailscale }),
            poll_interval: TAILNET_POLL_INTERVAL,
            show_btop: false,
        });
    }
    Ok(feeds)
}

/// Builds the feed of one host card.
fn host_feed_spec(
    host: &HostConfig,
    tailscale: &TailscaleStatusReader,
    agent: ureq::Agent,
) -> anyhow::Result<StatusFeedSpec> {
    let title = match host {
        HostConfig::Local { name }
        | HostConfig::Glances { name, .. }
        | HostConfig::Tailscale { name, .. } => name.trim(),
    };
    ensure!(!title.is_empty(), "host name must not be empty");
    match host {
        HostConfig::Local { .. } => Ok(local_feed_spec(title.into())),
        HostConfig::Glances {
            url,
            fallback_device,
            show_btop,
            ..
        } => Ok(StatusFeedSpec {
            title: title.into(),
            source: "Glances",
            collector: StatusCollector::Glances(GlancesCollector {
                base_url: source_url(url)
                    .with_context(|| format!("invalid Glances URL for {title}"))?,
                agent,
                presence: fallback_device.as_ref().map(|name| TailnetHostCollector {
                    reader: tailscale.clone(),
                    name: name.clone(),
                }),
            }),
            poll_interval: HOST_POLL_INTERVAL,
            show_btop: *show_btop,
        }),
        HostConfig::Tailscale { device, .. } => Ok(StatusFeedSpec {
            title: title.into(),
            source: "Tailscale",
            collector: StatusCollector::TailnetHost(TailnetHostCollector {
                reader: tailscale.clone(),
                name: device.clone(),
            }),
            poll_interval: TAILNET_POLL_INTERVAL,
            show_btop: false,
        }),
    }
}

fn source_url(value: &str) -> anyhow::Result<String> {
    let uri: Uri = value.parse().context("expected an HTTP or HTTPS URL")?;
    ensure!(
        matches!(uri.scheme_str(), Some("http" | "https"))
            && uri
                .authority()
                .is_some_and(|host| !host.as_str().contains('@'))
            && uri.query().is_none()
            && !value.contains('#'),
        "use an HTTP or HTTPS base URL without credentials, query, or fragment"
    );
    Ok(value.trim_end_matches('/').into())
}

/// Builds the feed of the machine that runs the display.
fn local_feed_spec(title: String) -> StatusFeedSpec {
    StatusFeedSpec {
        title,
        source: "local",
        collector: StatusCollector::Local(LocalCollector::default()),
        poll_interval: HOST_POLL_INTERVAL,
        show_btop: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_enables_only_local_metrics() {
        let config: StatusConfig =
            toml::from_str(include_str!("../../config.example.toml")).unwrap();
        let feeds = build_status_feeds(&config).unwrap();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].title, "This machine");
        assert_eq!(feeds[0].source, "local");
    }

    #[test]
    fn empty_config_shows_only_this_machine() {
        let config: StatusConfig = toml::from_str("").unwrap();
        assert_eq!(config, StatusConfig::default());
        let feeds = build_status_feeds(&config).unwrap();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].source, "local");
    }

    #[test]
    fn host_cards_keep_order_and_use_aliases_independent_of_sources() {
        let config: StatusConfig = toml::from_str(
            r#"
            [[host]]
            name = "Office server"
            source = "glances"
            url = "http://127.0.0.1:61208"

            [[host]]
            name = "Travel laptop"
            source = "tailscale"
            device = "laptop"

            [[host]]
            name = "Wall display"
            source = "local"
            "#,
        )
        .unwrap();
        let feeds = build_status_feeds(&config).unwrap();
        let cards: Vec<_> = feeds
            .iter()
            .map(|feed| {
                (
                    feed.title.as_str(),
                    feed.source,
                    feed.collector.tailnet_device_name(),
                )
            })
            .collect();
        assert_eq!(
            cards,
            [
                ("Office server", "Glances", None),
                ("Travel laptop", "Tailscale", Some("laptop")),
                ("Wall display", "local", None),
            ]
        );
    }

    #[test]
    fn rejects_fields_that_do_not_belong_to_the_selected_source() {
        let text = r#"
            [[host]]
            name = "x"
            source = "local"
            url = "http://127.0.0.1"
        "#;
        assert!(toml::from_str::<StatusConfig>(text).is_err());
    }
}
