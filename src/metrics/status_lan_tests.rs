use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use super::status_config::HostConfig;
use super::{FeedData, FeedFreshness, ServiceState, StatusConfig, StatusFeed, build_status_feeds};

struct HttpFixture {
    url: String,
    worker: Option<JoinHandle<()>>,
}

impl HttpFixture {
    fn start(reply: impl Fn(&str) -> Option<&'static str> + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            for connection in listener.incoming() {
                let mut stream = connection.unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).unwrap() == 0 {
                    break;
                }
                for header in reader.lines() {
                    if header.unwrap().is_empty() {
                        break;
                    }
                }
                let mut parts = request_line.split_whitespace();
                assert_eq!(parts.next(), Some("GET"));
                if let Some(response) = reply(parts.next().unwrap()) {
                    stream.write_all(response.as_bytes()).unwrap();
                }
            }
        });
        Self {
            url,
            worker: Some(worker),
        }
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        // An empty connection wakes the listener and ends its request loop.
        drop(TcpStream::connect(self.url.trim_start_matches("http://")));
        if let Some(worker) = self.worker.take()
            && let Err(error) = worker.join()
            && !thread::panicking()
        {
            std::panic::resume_unwind(error);
        }
    }
}

fn fixture_config(server: &HttpFixture, workstation: &HttpFixture) -> StatusConfig {
    let text = include_str!("../../config.lan.example.toml")
        .replace("http://192.0.2.10:61208", &server.url)
        .replace("http://192.0.2.10:3001", &server.url)
        .replace("http://192.0.2.10:8080", &server.url)
        .replace("http://192.0.2.20:61208", &workstation.url);
    toml::from_str(&text).unwrap()
}

fn metrics_reply(path: &str) -> &'static str {
    match path {
        "/api/4/cpu" => "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"total\":25}",
        "/api/4/mem" => "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"total\":1024,\"used\":256}",
        "/api/status-page/display" => concat!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
            r#"{"publicGroupList":[{"monitorList":[{"id":1,"name":"Test service"}]}]}"#,
        ),
        "/api/status-page/heartbeat/display" => concat!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
            r#"{"heartbeatList":{"1":[{"status":1,"time":"2026-01-01 00:00:00"}]}}"#,
        ),
        "/api/summary" => concat!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
            r#"{"data":{"summary":{"test-disk":{"device":{"device_name":"sda"}}}}}"#,
        ),
        _ => "HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
    }
}

fn workstation_reply(path: &str) -> &'static str {
    match path {
        "/api/4/mem" => "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"total\":2048,\"used\":768}",
        _ => metrics_reply(path),
    }
}

#[test]
fn lan_metrics_stay_live_when_tailnet_is_unavailable() {
    let server = HttpFixture::start(|path| Some(metrics_reply(path)));
    let workstation = HttpFixture::start(|path| Some(workstation_reply(path)));
    let mut config = fixture_config(&server, &workstation);
    let temp = tempfile::tempdir().unwrap();
    config.tailscale.command = temp.path().join("missing-tailscale").display().to_string();
    config.tailscale.show_devices = true;
    config.hosts.push(HostConfig::Tailscale {
        name: "Remote card".into(),
        device: "example-peer".into(),
    });
    let mut feeds = build_status_feeds(&config).unwrap().into_iter();
    let mut poll = || {
        let mut feed = StatusFeed::new_status_feed(feeds.next().unwrap());
        feed.record_feed_poll(feed.spec.collector.collect_status_reading());
        feed
    };
    let local = poll();
    let server = poll();
    let workstation = poll();
    let remote = poll();
    let services = poll();
    let disks = poll();
    let tailnet = poll();

    let [
        FeedData::Host(local),
        FeedData::Host(server),
        FeedData::Host(workstation),
        FeedData::Services(services),
        FeedData::Disks(disks),
    ] = [&local, &server, &workstation, &services, &disks].map(|feed| {
        assert_eq!(feed.feed_freshness(Duration::ZERO), FeedFreshness::Live);
        feed.latest_reading().unwrap()
    })
    else {
        panic!("expected local metrics, two Glances hosts, Kuma, and Scrutiny");
    };
    assert!(local.memory_total_bytes.is_some());
    assert_eq!(server.memory_used_bytes, Some(256));
    assert_eq!(workstation.memory_used_bytes, Some(768));
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].state, ServiceState::Up);
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].device, "sda");
    for feed in [&remote, &tailnet] {
        assert!(feed.last_poll_error().is_some());
        assert_eq!(
            feed.feed_freshness(feed.policy.offline_after),
            FeedFreshness::Offline
        );
    }
}

#[test]
fn lan_profile_rejects_redirects_even_with_a_valid_metrics_body() {
    let fixture = HttpFixture::start(|path| {
        assert_eq!(path, "/api/4/cpu");
        Some(concat!(
            "HTTP/1.1 302 Found\r\nLocation: /redirected\r\nConnection: close\r\n\r\n",
            r#"{"total":25}"#,
        ))
    });
    let feeds = build_status_feeds(&fixture_config(&fixture, &fixture)).unwrap();
    let error = feeds[1].collector.collect_status_reading().unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ureq::Error>(),
        Some(ureq::Error::StatusCode(302))
    ));
}

#[test]
fn lan_host_recovers_after_dropped_connections_without_affecting_other_feeds() {
    let online = std::sync::Arc::new(AtomicBool::new(false));
    let server = HttpFixture::start(|path| Some(metrics_reply(path)));
    let workstation = HttpFixture::start({
        let online = online.clone();
        move |path| {
            online
                .load(Ordering::Relaxed)
                .then(|| workstation_reply(path))
        }
    });
    let mut feeds = build_status_feeds(&fixture_config(&server, &workstation)).unwrap();
    let mut glances = StatusFeed::new_status_feed(feeds.remove(2));

    glances.record_feed_poll(glances.spec.collector.collect_status_reading());
    assert!(glances.last_poll_error().is_some());
    assert_eq!(
        glances.feed_freshness(glances.policy.offline_after),
        FeedFreshness::Offline
    );
    for feed in feeds {
        let reading = feed.collector.collect_status_reading();
        assert!(reading.is_ok(), "{} failed: {reading:?}", feed.title);
    }

    online.store(true, Ordering::Relaxed);
    glances.record_feed_poll(glances.spec.collector.collect_status_reading());

    assert!(glances.last_poll_error().is_none());
    assert_eq!(
        glances.feed_freshness(glances.policy.offline_after),
        FeedFreshness::Live
    );
    let Some(FeedData::Host(host)) = glances.latest_reading() else {
        panic!("expected a Glances reading after recovery");
    };
    assert_eq!(host.memory_used_bytes, Some(768));
}

#[test]
fn glances_requests_processes_only_for_btop_hosts() {
    let process_requests = Arc::new(AtomicUsize::new(0));
    let fixture = HttpFixture::start({
        let process_requests = process_requests.clone();
        move |path| {
            if path == "/api/4/processlist/top/16" {
                process_requests.fetch_add(1, Ordering::Relaxed);
                return Some(concat!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
                    r#"[{"pid":42,"name":"example","cpu_percent":3}]"#,
                ));
            }
            Some(metrics_reply(path))
        }
    });
    let mut config = fixture_config(&fixture, &fixture);
    let feeds = build_status_feeds(&config).unwrap();
    let first = feeds[1].collector.collect_status_reading().unwrap();
    assert!(matches!(first, FeedData::Host(host) if host.processes.is_empty()));
    assert_eq!(process_requests.load(Ordering::Relaxed), 0);

    let HostConfig::Glances { show_btop, .. } = &mut config.hosts[1] else {
        panic!("expected a Glances fixture");
    };
    *show_btop = true;
    let feeds = build_status_feeds(&config).unwrap();
    let second = feeds[1].collector.collect_status_reading().unwrap();
    assert!(matches!(second, FeedData::Host(host) if host.processes[0].pid == 42));
    assert_eq!(process_requests.load(Ordering::Relaxed), 1);
}
