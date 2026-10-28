//! Tests for the bounded event bus.

use super::{AppEvent, EventBus, ProfileChangeReason, MAX_EVENT_CAPACITY};
use serde_json::json;

fn resync(n: u64) -> AppEvent {
    AppEvent::Resync { missed: n }
}

#[tokio::test]
async fn fanout_preserves_ordering_across_subscribers() {
    let bus = EventBus::default();
    let mut first = bus.subscribe();
    let mut second = bus.subscribe();

    for n in 1..=3 {
        assert_eq!(bus.publish(resync(n)), 2);
    }

    for n in 1..=3 {
        assert_eq!(first.recv().await, Some(resync(n)));
        assert_eq!(second.recv().await, Some(resync(n)));
    }
}

#[tokio::test]
async fn lag_yields_resync_then_retained_tail() {
    let bus = EventBus::new(2).unwrap();
    let mut subscriber = bus.subscribe();

    for n in 101..=104u64 {
        bus.publish(resync(n));
    }

    // Capacity 2 retains 103 and 104; 101 and 102 were dropped.
    assert_eq!(subscriber.recv().await, Some(resync(2)), "lag report");
    assert_eq!(subscriber.recv().await, Some(resync(103)));
    assert_eq!(subscriber.recv().await, Some(resync(104)));
}

#[test]
fn zero_capacity_is_rejected() {
    let error = EventBus::new(0).unwrap_err();
    assert_eq!(
        error.kind(),
        crate::AppErrorKind::Validation {
            field: "event_capacity"
        }
    );
}

#[test]
fn oversized_capacity_is_rejected() {
    let error = EventBus::new(MAX_EVENT_CAPACITY + 1).unwrap_err();
    assert_eq!(
        error.kind(),
        crate::AppErrorKind::Validation {
            field: "event_capacity"
        }
    );
}

#[test]
fn publish_without_subscribers_is_not_an_error() {
    assert_eq!(EventBus::default().publish(resync(1)), 0);
}

#[tokio::test]
async fn recv_returns_none_when_bus_is_gone() {
    let bus = EventBus::default();
    let mut subscriber = bus.subscribe();
    drop(bus);
    assert_eq!(subscriber.recv().await, None);
}

#[tokio::test]
async fn startup_completion_preserves_kind_for_all_subscribers() {
    use crate::StartupJobKind;
    let bus = EventBus::default();
    let mut receiver = bus.subscribe();
    let kinds = [
        StartupJobKind::HealthBatch,
        StartupJobKind::OfflineReadiness,
        StartupJobKind::VersionScan,
        StartupJobKind::UmuRefresh,
        StartupJobKind::BackgroundPortal,
    ];
    for job in kinds {
        let event = AppEvent::StartupJobFinished { job };
        bus.publish(event.clone());
        assert_eq!(receiver.recv().await, Some(event));
    }
}

#[test]
fn profile_change_reasons_have_tagged_wire_shapes() {
    let cases = [
        (ProfileChangeReason::Saved, json!({"kind": "saved"})),
        (ProfileChangeReason::Deleted, json!({"kind": "deleted"})),
        (
            ProfileChangeReason::Renamed {
                from: "Alpha".into(),
            },
            json!({"kind": "renamed", "from": "Alpha"}),
        ),
        (
            ProfileChangeReason::Duplicated,
            json!({"kind": "duplicated"}),
        ),
        (
            ProfileChangeReason::FavoriteToggled,
            json!({"kind": "favorite_toggled"}),
        ),
        (
            ProfileChangeReason::RolledBack,
            json!({"kind": "rolled_back"}),
        ),
        (
            ProfileChangeReason::PresetApplied,
            json!({"kind": "preset_applied"}),
        ),
        (ProfileChangeReason::Imported, json!({"kind": "imported"})),
        (ProfileChangeReason::Migrated, json!({"kind": "migrated"})),
    ];
    for (reason, expected) in cases {
        assert_eq!(serde_json::to_value(&reason).unwrap(), expected);
    }
}

#[tokio::test]
async fn profiles_changed_round_trips_through_the_bus() {
    let bus = EventBus::default();
    let mut receiver = bus.subscribe();
    let event = AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Renamed {
            from: "Alpha".into(),
        },
        names: vec!["Beta".into()],
    };
    bus.publish(event.clone());
    assert_eq!(receiver.recv().await, Some(event));
}

#[tokio::test]
async fn lagging_profile_subscriber_gets_resync_then_tail() {
    let bus = EventBus::new(1).unwrap();
    let mut receiver = bus.subscribe();
    let changed = |name: &str| AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Saved,
        names: vec![name.to_string()],
    };
    bus.publish(changed("Alpha"));
    bus.publish(changed("Beta"));

    assert_eq!(receiver.recv().await, Some(resync(1)));
    assert_eq!(receiver.recv().await, Some(changed("Beta")));
}

#[tokio::test]
async fn named_scan_events_round_trip_and_carry_counts_only() {
    let bus = EventBus::default();
    let mut receiver = bus.subscribe();
    let events = [
        AppEvent::HealthScanCompleted {
            total: 6,
            healthy: 3,
            stale: 2,
            broken: 1,
        },
        AppEvent::OfflineReadinessScanCompleted { total_profiles: 6 },
        AppEvent::VersionScanCompleted {
            scanned: 4,
            mismatches: 1,
        },
    ];
    for event in &events {
        bus.publish(event.clone());
    }
    for event in events {
        assert_eq!(receiver.recv().await, Some(event));
    }
}
