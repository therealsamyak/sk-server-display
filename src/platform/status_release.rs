//! Discover public GitHub releases and select the package for this device.

use std::time::Duration;

use anyhow::{Context as _, ensure};
use semver::Version;
use serde::Deserialize;
use ureq::{Agent, Body, http::Response};

const RELEASE_API: &str =
    "https://api.github.com/repos/therealsamyak/server-status-display/releases/latest";
const RELEASE_DOWNLOADS: &str =
    "https://github.com/therealsamyak/server-status-display/releases/download";
const MAX_BUILD_SIZE: u64 = 1024 * 1024 * 1024;

/// Result of checking the latest public release for this device.
pub(crate) enum ReleaseCheck {
    Unavailable,
    Current,
    Available(ReleaseUpdate),
}

/// A newer build and the digest GitHub reports for its release asset.
pub(crate) struct ReleaseUpdate {
    pub(crate) version: Version,
    pub(crate) size: u64,
    pub(crate) sha256: String,
    url: String,
    agent: Agent,
}

impl ReleaseUpdate {
    /// Open the selected release binary for streaming verification.
    pub(crate) fn download_release_asset(&self) -> anyhow::Result<Response<Body>> {
        self.agent
            .get(&self.url)
            .call()
            .context("Could not download the release build.")
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    size: u64,
    digest: Option<String>,
}

/// Check GitHub for an anonymous public release that updates the installed build.
pub(crate) fn check_display_release() -> anyhow::Result<ReleaseCheck> {
    #[cfg(debug_assertions)]
    let fixture = std::env::var("STATUS_DISPLAY_UPDATE_FIXTURE_PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .map(|port| format!("http://127.0.0.1:{port}"));
    #[cfg(not(debug_assertions))]
    let fixture: Option<String> = None;
    let api = fixture.as_ref().map(|root| format!("{root}/latest"));
    let downloads = fixture.as_ref().map(|root| format!("{root}/download"));
    let agent = Agent::config_builder()
        .https_only(fixture.is_none())
        .proxy(None)
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_recv_response(Some(Duration::from_secs(10)))
        .timeout_recv_body(Some(Duration::from_secs(120)))
        .max_redirects(4)
        .user_agent(concat!("ServerStatusDisplay/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let current =
        Version::parse(env!("CARGO_PKG_VERSION")).context("Invalid installed version.")?;
    let asset = asset_name(std::env::consts::OS, std::env::consts::ARCH);
    check_release(
        agent,
        api.as_deref().unwrap_or(RELEASE_API),
        downloads.as_deref().unwrap_or(RELEASE_DOWNLOADS),
        &current,
        &asset,
    )
}

fn asset_name(os: &str, arch: &str) -> String {
    let extension = if os == "windows" { ".exe" } else { "" };
    format!("server-status-display-{os}-{arch}{extension}")
}

fn check_release(
    agent: Agent,
    api: &str,
    downloads: &str,
    installed: &Version,
    asset: &str,
) -> anyhow::Result<ReleaseCheck> {
    let response = agent
        .get(api)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call();
    let mut response = match response {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(ReleaseCheck::Unavailable),
        Err(error) => return Err(error).context("Could not check GitHub for updates."),
    };
    let release = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json::<GithubRelease>()
        .context("Could not read the latest release.")?;
    if release.draft || release.prerelease {
        return Ok(ReleaseCheck::Unavailable);
    }
    let tag = &release.tag_name;
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .context("The release tag is not a version number.")?;
    if version.cmp_precedence(installed).is_le() {
        return Ok(ReleaseCheck::Current);
    }
    let binary = release
        .assets
        .into_iter()
        .find(|candidate| candidate.name == asset)
        .with_context(|| format!("Version {version} has no build for this device."))?;
    let digest = binary
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .context("GitHub did not supply a SHA-256 digest for the release build.")?;
    ensure!(
        (1..=MAX_BUILD_SIZE).contains(&binary.size),
        "The release build has an invalid size."
    );
    Ok(ReleaseCheck::Available(ReleaseUpdate {
        version,
        size: binary.size,
        sha256: digest.to_owned(),
        url: format!("{downloads}/{tag}/{asset}"),
        agent,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{BufRead as _, BufReader, Write as _},
        net::TcpListener,
        thread,
    };

    fn mocked_release(
        status: &str,
        body: serde_json::Value,
        version: &str,
    ) -> anyhow::Result<ReleaseCheck> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let wire = body.to_string();
        let status = status.to_owned();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = BufReader::new(&mut stream);
            let mut line = String::new();
            request.read_line(&mut line).unwrap();
            assert!(line.starts_with("GET /latest "));
            while line != "\r\n" {
                line.clear();
                request.read_line(&mut line).unwrap();
            }
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{wire}",
                wire.len()
            ).unwrap();
        });
        let agent = Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(Duration::from_secs(3)))
            .build()
            .into();
        let result = check_release(
            agent,
            &format!("{root}/latest"),
            &format!("{root}/download"),
            &Version::parse(version).unwrap(),
            &asset_name("windows", "aarch64"),
        );
        server.join().unwrap();
        result
    }

    #[test]
    fn selects_only_the_matching_newer_asset() {
        let digest = "f".repeat(64);
        let release = json!({"tag_name": "v0.12.0", "draft": false, "prerelease": false, "assets": [
            {"name": "server-status-display-windows-x86_64.exe", "size": 10, "digest": format!("sha256:{digest}")},
            {"name": "server-status-display-windows-aarch64.exe", "size": 20, "digest": format!("sha256:{digest}")}
        ]});
        let ReleaseCheck::Available(update) =
            mocked_release("200 OK", release.clone(), "0.9.0").unwrap()
        else {
            panic!("expected an available ARM64 update");
        };
        assert_eq!(update.version, Version::new(0, 12, 0));
        assert_eq!(update.sha256, digest);
        assert_eq!(update.size, 20);
        assert!(
            update
                .url
                .ends_with("/v0.12.0/server-status-display-windows-aarch64.exe")
        );
        assert!(matches!(
            mocked_release("200 OK", release, "0.12.0").unwrap(),
            ReleaseCheck::Current
        ));
    }

    #[test]
    fn reports_an_unavailable_private_release() {
        assert!(matches!(
            mocked_release("404 Not Found", json!({"message": "Not Found"}), "0.1.0").unwrap(),
            ReleaseCheck::Unavailable
        ));
    }

    #[test]
    fn rejects_a_newer_asset_without_a_trusted_digest() {
        let release = json!({"tag_name": "v1.0.0", "draft": false, "prerelease": false, "assets": [
            {"name": "server-status-display-windows-aarch64.exe", "size": 20, "digest": null}
        ]});
        let Err(error) = mocked_release("200 OK", release, "0.1.0") else {
            panic!("expected a missing digest error");
        };
        assert!(error.to_string().contains("SHA-256"));
    }
}
