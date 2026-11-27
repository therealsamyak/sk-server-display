use crate::metrics::{
    FeedData, StatusConfig, StatusFeed, TailnetConnection, TailnetDevice, build_status_feeds,
};

use super::{CardedTailnetDevices, list_panel_height, side_panel_height};

#[test]
fn tailnet_height_excludes_self_when_local_card_is_configured() {
    let feeds = fixture_feeds(true);
    let carded = CardedTailnetDevices::from_status_feeds(&feeds);

    let height = side_panel_height(feeds.last().unwrap(), &carded);

    assert!((height - list_panel_height(1)).abs() < f32::EPSILON);
}

#[test]
fn tailnet_peers_remain_visible_when_offline_local_card_is_configured() {
    let mut feeds = fixture_feeds(true);
    feeds[0].record_feed_poll(Err(anyhow::anyhow!("test local metrics unavailable")));
    let carded = CardedTailnetDevices::from_status_feeds(&feeds);
    let Some(FeedData::Tailnet(devices)) = feeds.last().unwrap().latest_reading() else {
        panic!("fixture must have a Tailnet reading");
    };

    let names: Vec<_> = carded
        .visible_devices(devices)
        .map(|device| device.name.as_str())
        .collect();

    assert_eq!(names, ["remote-device"]);
}

#[test]
fn tailnet_self_remains_visible_without_a_local_card() {
    let feeds = fixture_feeds(false);
    let carded = CardedTailnetDevices::from_status_feeds(&feeds);
    let feed = feeds.last().unwrap();
    let Some(FeedData::Tailnet(devices)) = feed.latest_reading() else {
        panic!("fixture must have a Tailnet reading");
    };

    let names: Vec<_> = carded
        .visible_devices(devices)
        .map(|device| device.name.as_str())
        .collect();

    assert_eq!(names, ["actual-self", "remote-device"]);
    assert!((side_panel_height(feed, &carded) - list_panel_height(2)).abs() < f32::EPSILON);
}

fn fixture_feeds(local_card: bool) -> Vec<StatusFeed> {
    let local = if local_card {
        "[[host]]\nname = 'remote-device'\nsource = 'local'\n"
    } else {
        ""
    };
    let config: StatusConfig = toml::from_str(&format!(
        "{local}\n[[host]]\nname = 'Named remote card'\nsource = 'tailscale'\ndevice = 'CARDED-REMOTE'\n\
         [tailscale]\nshow_devices = true"
    ))
    .unwrap();
    let mut feeds = build_status_feeds(&config)
        .unwrap()
        .into_iter()
        .map(StatusFeed::new_status_feed)
        .collect::<Vec<_>>();
    feeds
        .last_mut()
        .unwrap()
        .record_feed_poll(Ok(FeedData::Tailnet(vec![
            fixture_device("actual-self", TailnetConnection::ThisMachine),
            fixture_device("remote-device", TailnetConnection::Idle),
            fixture_device("carded-remote", TailnetConnection::Idle),
        ])));
    feeds
}

fn fixture_device(name: &str, connection: TailnetConnection) -> TailnetDevice {
    TailnetDevice {
        name: name.into(),
        os: "windows".into(),
        online: true,
        is_self: connection == TailnetConnection::ThisMachine,
        last_seen: None,
        connection,
    }
}
