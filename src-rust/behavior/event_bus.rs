//! Event-driven distribution for the companion brain.
//!
//! Events only *describe what happened* — they never decide how the pet should
//! respond. The [`EventBus`] broadcasts normalized [`PetEvent`](super::event::PetEvent)
//! values to every registered [`EventSubscriber`]; each subscriber (state engine,
//! emotion engine, behavior planner, ...) decides for itself how to react. Adding
//! a new event variant therefore never requires touching the behavior system,
//! only the subscribers that care about it.

use super::event::PetEvent;

/// A time-stamped observer of normalized pet events.
///
/// Subscribers receive every published event together with the logical clock
/// (`now_ms`) so they can advance their own decay or timers without owning a
/// clock. The trait is object-safe, so subscribers are stored on the bus as
/// `Box<dyn EventSubscriber>`.
pub trait EventSubscriber {
    fn on_event(&mut self, event: &PetEvent, now_ms: u64);
}

/// Opaque handle returned by [`EventBus::subscribe`], used to later remove a
/// subscriber. It is cheap to copy and compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(usize);

/// A broadcast dispatcher that decouples event production from consumption.
///
/// The bus owns its subscribers as trait objects and delivers events to all of
/// them in subscription order. Removal is handle-based so callers never need to
/// compare trait objects, which keeps the subscription set stable even when a
/// subscriber is dropped.
#[derive(Default)]
pub struct EventBus {
    subscribers: Vec<(SubscriptionId, Box<dyn EventSubscriber>)>,
    next_id: usize,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a subscriber and return the handle needed to remove it later.
    pub fn subscribe(&mut self, subscriber: Box<dyn EventSubscriber>) -> SubscriptionId {
        let id = SubscriptionId(self.next_id);
        self.next_id += 1;
        self.subscribers.push((id, subscriber));
        id
    }

    /// Remove a previously registered subscriber, returning ownership back to
    /// the caller. Returns `None` if the handle is unknown or already removed.
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> Option<Box<dyn EventSubscriber>> {
        let pos = self
            .subscribers
            .iter()
            .position(|(sid, _)| *sid == id)?;
        Some(self.subscribers.remove(pos).1)
    }

    /// Broadcast an event to every active subscriber, in subscription order.
    pub fn publish(&mut self, event: &PetEvent, now_ms: u64) {
        for (_, subscriber) in &mut self.subscribers {
            subscriber.on_event(event, now_ms);
        }
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    pub fn clear(&mut self) {
        self.subscribers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    /// A subscriber whose only effect is bumping a shared counter, so tests can
    /// observe delivery without downcasting the boxed trait object back out.
    #[derive(Debug)]
    struct CountingSubscriber {
        count: Rc<Cell<u32>>,
    }

    impl EventSubscriber for CountingSubscriber {
        fn on_event(&mut self, _event: &PetEvent, _now_ms: u64) {
            self.count.set(self.count.get() + 1);
        }
    }

    /// A subscriber that records the debug form of every event, to assert that
    /// the bus delivers the actual event content in subscription order.
    #[derive(Debug)]
    struct RecordingSubscriber {
        received: Rc<RefCell<Vec<String>>>,
    }

    impl EventSubscriber for RecordingSubscriber {
        fn on_event(&mut self, event: &PetEvent, _now_ms: u64) {
            self.received.borrow_mut().push(format!("{event:?}"));
        }
    }

    fn counting() -> (Box<CountingSubscriber>, Rc<Cell<u32>>) {
        let counter = Rc::new(Cell::new(0));
        let subscriber = Box::new(CountingSubscriber {
            count: counter.clone(),
        });
        (subscriber, counter)
    }

    #[test]
    fn test_subscribe_and_publish_broadcasts_to_all() {
        let mut bus = EventBus::new();
        let (a, a_count) = counting();
        let (b, b_count) = counting();
        let _ = bus.subscribe(a);
        let _ = bus.subscribe(b);
        assert_eq!(bus.subscriber_count(), 2);

        bus.publish(&PetEvent::AppStarted, 0);
        bus.publish(&PetEvent::PointerExited, 5);

        assert_eq!(a_count.get(), 2);
        assert_eq!(b_count.get(), 2);
    }

    #[test]
    fn test_unsubscribe_stops_delivery_and_is_idempotent() {
        let mut bus = EventBus::new();
        let (a, a_count) = counting();
        let (b, b_count) = counting();
        let a_id = bus.subscribe(a);
        let b_id = bus.subscribe(b);

        bus.publish(&PetEvent::AppStarted, 0);
        assert_eq!(a_count.get(), 1);

        let removed = bus.unsubscribe(a_id);
        assert!(removed.is_some());
        assert_eq!(bus.subscriber_count(), 1);

        // A no longer receives events after removal.
        bus.publish(&PetEvent::AppStarted, 1);
        assert_eq!(a_count.get(), 1);
        assert_eq!(b_count.get(), 2);

        // Removing the same handle twice yields nothing.
        assert!(bus.unsubscribe(a_id).is_none());
        // The still-subscribed handle remains valid.
        assert!(bus.unsubscribe(b_id).is_some());
    }

    #[test]
    fn test_publish_delivers_event_content_in_order() {
        let received = Rc::new(RefCell::new(Vec::new()));
        let mut bus = EventBus::new();
        let _ = bus.subscribe(Box::new(RecordingSubscriber {
            received: received.clone(),
        }));
        bus.publish(&PetEvent::AppStarted, 0);
        bus.publish(&PetEvent::PointerExited, 1);
        let messages = received.borrow();
        assert_eq!(messages.len(), 2);
        assert!(messages[0].contains("AppStarted"));
        assert!(messages[1].contains("PointerExited"));
    }

    #[test]
    fn test_publish_to_empty_bus_is_a_noop() {
        let mut bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);
        bus.publish(&PetEvent::AppStarted, 0);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_clear_removes_every_subscriber() {
        let mut bus = EventBus::new();
        let (a, _) = counting();
        let (b, _) = counting();
        let _ = bus.subscribe(a);
        let _ = bus.subscribe(b);
        bus.clear();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_state_engine_plugs_into_bus() {
        // The numeric state model implements EventSubscriber, so it type-checks
        // as a bus subscriber and is driven by publish without panic. The actual
        // state reaction is covered by state_model's own tests.
        use super::super::event::HitRegion;
        use super::super::state_model::PetStats;

        let mut bus = EventBus::new();
        let id = bus.subscribe(Box::<PetStats>::default());
        assert_eq!(bus.subscriber_count(), 1);
        bus.publish(
            &PetEvent::PetClicked {
                region: HitRegion::Head,
                click_count: 1,
            },
            0,
        );
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id).is_some());
    }
}

