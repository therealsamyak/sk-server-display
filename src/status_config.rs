//! Status display configuration: which hosts and services to read.
//!
//! The file is TOML. It stays on the display machine and never goes into
//! Git, because it names private hosts. See `config.example.toml`.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use serde::Deserialize;

use crate::{
    collector_glances::GlancesCollector,
    collector_kuma::KumaCollector,
    collector_local::LocalCollector,
    collector_scrutiny::ScrutinyCollector,
    collector_tailscale::{TailnetHostCollector, TailscaleCollector, TailscaleStatusReader},
    status_collector::StatusCollector,
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
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StatusConfig {
    /// Background image of the status display. A relative path starts from
    /// the folder of the configuration file. Without it the background is
    /// a solid color.
    #[serde(default)]
    pub wallpaper: Option<PathBuf>,
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

/// One host card. The source of its data is the first that applies:
/// `glances_url`, then `this_machine`, then the Tailscale presence of the
/// device with this name.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    /// Name on the host card. For a Tailscale-only host, the first label of
    /// its MagicDNS name, such as `server`.
    pub name: String,
    /// Glances web server URL, such as `http://server.example.ts.net:61208`.
    #[serde(default)]
    pub glances_url: Option<String>,
    /// Reads the metrics of the machine that runs the display.
    #[serde(default)]
    pub this_machine: bool,
}

/// Uptime Kuma public status page.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KumaConfig {
    /// Uptime Kuma URL, such as `http://server.example.ts.net:3001`.
    pub url: String,
    /// Status page slug.
    pub status_page: String,
}

/// Scrutiny web server.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScrutinyConfig {
    /// Scrutiny URL, such as `http://server.example.ts.net:8080`.
    pub url: String,
}

/// Tailscale device list.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TailscaleConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Tailscale command name or path.
    #[serde(default = "default_tailscale_command")]
    pub command: String,
}

impl Default for TailscaleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            command: default_tailscale_command(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_tailscale_command() -> String {
    "tailscale".into()
}

/// Default configuration file path, such as
/// `%APPDATA%\server-status-display\config.toml` on Windows.
pub fn default_status_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("server-status-display").join("config.toml"))
}

/// Reads the configuration. A missing default file gives the default
/// configuration: this machine and Tailscale only.
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
    if let (Some(wallpaper), Some(folder)) = (&config.wallpaper, path.parent()) {
        config.wallpaper = Some(folder.join(wallpaper));
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
}

/// Builds the status feeds in display order.
pub fn build_status_feeds(config: &StatusConfig) -> anyhow::Result<Vec<StatusFeedSpec>> {
    let agent = |interval: Duration| -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(
                interval
                    .mul_f32(0.75)
                    .clamp(Duration::from_millis(1500), Duration::from_secs(10)),
            ))
            .build()
            .into()
    };
    // One reader for all Tailscale feeds, so they share each command run.
    let tailscale = TailscaleStatusReader::new_tailscale_reader(config.tailscale.command.clone());
    let mut feeds = Vec::new();
    for host in &config.hosts {
        feeds.push(host_feed_spec(host, &tailscale, agent(HOST_POLL_INTERVAL))?);
    }
    if config.hosts.is_empty() {
        let name = sysinfo::System::host_name().unwrap_or_else(|_| "this machine".into());
        feeds.push(local_feed_spec(name)?);
    }
    if let Some(kuma) = &config.uptime_kuma {
        feeds.push(StatusFeedSpec {
            title: "Services".into(),
            source: "Uptime Kuma",
            collector: StatusCollector::Kuma(KumaCollector {
                base_url: kuma.url.clone(),
                status_page: kuma.status_page.clone(),
                agent: agent(SERVICE_POLL_INTERVAL),
            }),
            poll_interval: SERVICE_POLL_INTERVAL,
        });
    }
    if let Some(scrutiny) = &config.scrutiny {
        feeds.push(StatusFeedSpec {
            title: "Disks".into(),
            source: "Scrutiny",
            collector: StatusCollector::Scrutiny(ScrutinyCollector {
                base_url: scrutiny.url.clone(),
                agent: agent(DISK_POLL_INTERVAL),
            }),
            poll_interval: DISK_POLL_INTERVAL,
        });
    }
    if config.tailscale.enabled {
        feeds.push(StatusFeedSpec {
            title: "Tailnet".into(),
            source: "Tailscale",
            collector: StatusCollector::Tailscale(TailscaleCollector { reader: tailscale }),
            poll_interval: TAILNET_POLL_INTERVAL,
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
    if host.glances_url.is_some() && host.this_machine {
        anyhow::bail!(
            "host {} sets both glances_url and this_machine; keep one",
            host.name
        );
    }
    if let Some(url) = &host.glances_url {
        return Ok(StatusFeedSpec {
            title: host.name.clone(),
            source: "Glances",
            collector: StatusCollector::Glances(GlancesCollector {
                name: host.name.clone(),
                base_url: url.clone(),
                agent,
                tailscale: tailscale.clone(),
            }),
            poll_interval: HOST_POLL_INTERVAL,
        });
    }
    if host.this_machine {
        return local_feed_spec(host.name.clone());
    }
    Ok(StatusFeedSpec {
        title: host.name.clone(),
        source: "Tailscale",
        collector: StatusCollector::TailnetHost(TailnetHostCollector {
            reader: tailscale.clone(),
            name: host.name.clone(),
        }),
        poll_interval: TAILNET_POLL_INTERVAL,
    })
}

/// Builds the feed of the machine that runs the display.
fn local_feed_spec(title: String) -> anyhow::Result<StatusFeedSpec> {
    Ok(StatusFeedSpec {
        title,
        source: "local",
        collector: StatusCollector::Local(LocalCollector::new_local_collector()?),
        poll_interval: HOST_POLL_INTERVAL,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_config() {
        let text = include_str!("../config.example.toml");
        let config: StatusConfig = toml::from_str(text).expect("example config parses");
        let sources: Vec<_> = config
            .hosts
            .iter()
            .map(|host| (host.glances_url.is_some(), host.this_machine))
            .collect();
        assert_eq!(sources, [(true, false), (true, false), (false, false)]);
        assert_eq!(config.uptime_kuma.as_ref().unwrap().status_page, "home");
        assert!(config.scrutiny.is_some());
        assert_eq!(config.tailscale.command, "tailscale");
    }

    #[test]
    fn empty_config_shows_this_machine_and_tailnet() {
        let config: StatusConfig = toml::from_str("").unwrap();
        assert_eq!(config, StatusConfig::default());
        let feeds = build_status_feeds(&config).unwrap();
        let sources: Vec<_> = feeds.iter().map(|feed| feed.source).collect();
        assert_eq!(sources, ["local", "Tailscale"]);
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<StatusConfig>("glances_url = \"x\"").is_err());
    }

    #[test]
    fn host_cards_keep_config_order_and_sources() {
        let config: StatusConfig = toml::from_str(
            r#"
            [[host]]
            name = "server"
            glances_url = "http://server.example.ts.net:61208"

            [[host]]
            name = "desk"

            [[host]]
            name = "laptop"
            this_machine = true
            "#,
        )
        .unwrap();
        let feeds = build_status_feeds(&config).unwrap();
        let cards: Vec<_> = feeds
            .iter()
            .map(|feed| (feed.title.as_str(), feed.source))
            .collect();
        assert_eq!(
            cards,
            [
                ("server", "Glances"),
                ("desk", "Tailscale"),
                ("laptop", "local"),
                ("Tailnet", "Tailscale"),
            ]
        );
    }

    #[test]
    fn rejects_two_sources_for_one_host() {
        let config: StatusConfig = toml::from_str(
            "[[host]]\nname = \"x\"\nglances_url = \"http://x\"\nthis_machine = true\n",
        )
        .unwrap();
        assert!(build_status_feeds(&config).is_err());
    }
}
