//! Tests for the bounded event bus.

use super::{AppEvent, EventBus, MAX_EVENT_CAPACITY};

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
