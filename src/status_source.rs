//! Status source: where the display reads status snapshots from.

use anyhow::Context as _;

use crate::{
    status_demo::DemoStatusSource, status_freshness::FreshnessPolicy,
    status_snapshot::StatusSnapshot,
};

/// Status source: an HTTP JSON endpoint or the built-in demo data.
#[derive(Clone, Debug)]
pub enum StatusSource {
    /// Polls one URL that returns a `StatusSnapshot` JSON document.
    Http { url: String, agent: ureq::Agent },
    /// Generates sample snapshots in process.
    Demo(DemoStatusSource),
}

impl StatusSource {
    /// Creates an HTTP status source with the request timeout from `policy`.
    pub fn http_status_source(url: String, policy: &FreshnessPolicy) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(policy.request_timeout))
            .build()
            .into();
        Self::Http { url, agent }
    }

    /// Short label for the footer, such as `demo` or the endpoint host.
    pub fn status_source_label(&self) -> String {
        match self {
            Self::Http { url, .. } => url.clone(),
            Self::Demo(_) => "built-in demo data".into(),
        }
    }

    /// Fetches one snapshot. This call blocks; run it on a background thread.
    pub fn fetch_status_snapshot(&self) -> anyhow::Result<StatusSnapshot> {
        match self {
            Self::Http { url, agent } => agent
                .get(url)
                .call()
                .with_context(|| format!("status request failed: {url}"))?
                .body_mut()
                .read_json::<StatusSnapshot>()
                .context("status response is not a valid status snapshot"),
            Self::Demo(source) => source.fetch_demo_snapshot(),
        }
    }
}
