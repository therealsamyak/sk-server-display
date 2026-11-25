use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
    time::Duration,
};

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

fn fixture_config(fixture: &HttpFixture) -> StatusConfig {
    let text = include_str!("../../config.lan.example.toml")
        .replace("http://192.0.2.10:61208", &fixture.url)
        .replace("http://192.0.2.10:3001", &fixture.url)
        .replace("http://192.0.2.10:8080", &fixture.url);
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

#[test]
fn lan_profile_reads_all_metrics_from_local_routes() {
    let fixture = HttpFixture::start(|path| Some(metrics_reply(path)));
    let feeds = build_status_feeds(&fixture_config(&fixture)).unwrap();
    let data: Vec<_> = feeds
        .iter()
        .map(|feed| feed.collector.collect_status_reading().unwrap())
        .collect();

    let [
        FeedData::Host(local),
        FeedData::Host(remote),
        FeedData::Services(services),
        FeedData::Disks(disks),
    ] = data.as_slice()
    else {
        panic!("expected local, Glances, Kuma, and Scrutiny readings");
    };
    assert!(local.memory_total_bytes.is_some());
    assert_eq!(remote.memory_used_bytes, Some(256));
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].state, ServiceState::Up);
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].device, "sda");
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
    let feeds = build_status_feeds(&fixture_config(&fixture)).unwrap();
    let error = feeds[1].collector.collect_status_reading().unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ureq::Error>(),
        Some(ureq::Error::StatusCode(302))
    ));
}

#[test]
fn lan_feed_recovers_after_connections_drop() {
    let online = std::sync::Arc::new(AtomicBool::new(false));
    let fixture = HttpFixture::start({
        let online = online.clone();
        move |path| online.load(Ordering::Relaxed).then(|| metrics_reply(path))
    });
    let mut feeds = build_status_feeds(&fixture_config(&fixture)).unwrap();
    let mut glances = StatusFeed::new_status_feed(feeds.remove(1));

    glances.record_feed_poll(glances.spec.collector.collect_status_reading());
    assert!(glances.last_poll_error().is_some());
    assert_eq!(
        glances.feed_freshness(glances.policy.offline_after),
        FeedFreshness::Offline
    );
    assert!(matches!(
        feeds[0].collector.collect_status_reading().unwrap(),
        FeedData::Host(_)
    ));

    online.store(true, Ordering::Relaxed);
    glances.record_feed_poll(glances.spec.collector.collect_status_reading());

    assert!(glances.last_poll_error().is_none());
    assert_eq!(
        glances.feed_freshness(glances.policy.offline_after),
        FeedFreshness::Live
    );
    assert!(matches!(glances.latest_reading(), Some(FeedData::Host(_))));
}
