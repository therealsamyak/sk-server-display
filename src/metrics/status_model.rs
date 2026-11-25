//! Status model: the readings that the status collectors produce.

use time::OffsetDateTime;

/// Host metrics from one machine: CPU, memory, disk, temperature, and uptime.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostMetrics {
    pub cpu_percent: Option<f32>,
    /// Usage of each logical CPU core, in core order.
    pub core_percents: Vec<f32>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub disk_used_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub temperature_celsius: Option<f32>,
    pub load_1m: Option<f32>,
    pub uptime_seconds: Option<u64>,
    /// Network receive rate of the main interface, in bytes per second.
    pub network_receive_rate: Option<f64>,
    /// Network send rate of the main interface, in bytes per second.
    pub network_send_rate: Option<f64>,
}

/// Service health state, as Uptime Kuma reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    Up,
    Down,
    Pending,
    Maintenance,
    /// No heartbeat yet.
    Unknown,
}

/// Service health: one Uptime Kuma monitor on a status page.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceHealth {
    pub name: String,
    pub state: ServiceState,
    /// Uptime ratio over 24 hours, from 0.0 to 1.0.
    pub uptime_24h: Option<f32>,
    pub last_heartbeat: Option<OffsetDateTime>,
    /// Message of the last heartbeat, such as an error text.
    pub message: Option<String>,
}

/// Disk health state, as Scrutiny reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskState {
    Passed,
    /// The drive's own SMART check failed.
    FailedSmart,
    /// A SMART attribute crossed a Scrutiny failure threshold.
    FailedScrutiny,
    /// Both checks failed.
    FailedBoth,
}

/// Disk health: SMART status of one drive.
#[derive(Clone, Debug, PartialEq)]
pub struct DiskHealth {
    pub device: String,
    pub model: String,
    pub capacity_bytes: u64,
    pub state: DiskState,
    pub temperature_celsius: Option<f32>,
    pub power_on_hours: Option<u64>,
    pub collected_at: Option<OffsetDateTime>,
}

/// Tailnet device: one machine in the Tailscale network.
#[derive(Clone, Debug, PartialEq)]
pub struct TailnetDevice {
    pub name: String,
    pub os: String,
    pub online: bool,
    /// `true` for the machine that runs the status display.
    pub is_self: bool,
    pub last_seen: Option<OffsetDateTime>,
    pub connection: TailnetConnection,
}

/// How the status display machine reaches a tailnet device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TailnetConnection {
    /// This machine.
    ThisMachine,
    /// Direct peer-to-peer path.
    Direct,
    /// Traffic goes through a DERP relay, such as `lax`.
    Relay(String),
    /// Online, but no traffic now.
    Idle,
    Offline,
}

/// One reading from one status collector.
#[derive(Clone, Debug, PartialEq)]
pub enum FeedData {
    Host(HostMetrics),
    Services(Vec<ServiceHealth>),
    Disks(Vec<DiskHealth>),
    Tailnet(Vec<TailnetDevice>),
    /// One host card without metrics: its Tailscale presence.
    HostPresence(HostPresence),
}

/// Host presence: a host card from Tailscale, for a host that gives no
/// metrics now.
#[derive(Clone, Debug, PartialEq)]
pub struct HostPresence {
    pub device: TailnetDevice,
    /// Why the metrics are missing, such as a failed Glances request. `None`
    /// for a host without a metrics server.
    pub metrics_error: Option<String>,
}
