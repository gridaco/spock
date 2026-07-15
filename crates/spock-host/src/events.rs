use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde::Serialize;

pub const PROJECT_EVENT_PROTOCOL: &str = "spock-project-event/1";
pub const PROJECT_STATUS_PATH: &str = "/~project/status";
pub const MAX_EVENT_STREAMS_PER_SESSION: usize = 4;

/// One invalidation for the authoritative project-status snapshot.
///
/// Events deliberately carry no duplicated status fields. A browser that
/// reconnects, misses an event, or sees an unfamiliar ID always converges by
/// fetching `status_url`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectEvent {
    pub protocol: &'static str,
    pub event_id: u64,
    pub status_url: &'static str,
}

impl ProjectEvent {
    fn new(event_id: u64) -> Self {
        Self {
            protocol: PROJECT_EVENT_PROTOCOL,
            event_id,
            status_url: PROJECT_STATUS_PATH,
        }
    }

    fn sse_frame(&self) -> String {
        let data = serde_json::to_string(self).expect("project event always serializes");
        format!("id: {}\nevent: invalidate\ndata: {data}\n\n", self.event_id)
    }
}

/// Stable, host-session event hub shared across client publications.
#[derive(Default)]
struct HubState {
    current_id: u64,
    next_client_id: u64,
    clients: BTreeMap<u64, SyncSender<String>>,
}

pub struct ProjectEventHub {
    state: Arc<Mutex<HubState>>,
    admission: Arc<EventAdmission>,
}

impl Default for ProjectEventHub {
    fn default() -> Self {
        Self::with_admission(Arc::new(EventAdmission::new(MAX_EVENT_STREAMS_PER_SESSION)))
    }
}

pub(crate) struct EventAdmission {
    active: Mutex<usize>,
    limit: usize,
}

impl EventAdmission {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            active: Mutex::new(0),
            limit,
        }
    }

    pub(crate) fn try_acquire(self: &Arc<Self>) -> Option<EventStreamPermit> {
        let mut active = self.active.lock().expect("event admission lock");
        if *active >= self.limit {
            return None;
        }
        *active += 1;
        Some(EventStreamPermit {
            admission: Arc::clone(self),
        })
    }

    #[cfg(test)]
    fn active(&self) -> usize {
        *self.active.lock().expect("event admission lock")
    }
}

pub(crate) struct EventStreamPermit {
    admission: Arc<EventAdmission>,
}

impl Drop for EventStreamPermit {
    fn drop(&mut self) {
        let mut active = self.admission.active.lock().expect("event admission lock");
        *active = active
            .checked_sub(1)
            .expect("event admission count underflow");
    }
}

impl ProjectEventHub {
    pub(crate) fn with_admission(admission: Arc<EventAdmission>) -> Self {
        Self {
            state: Arc::new(Mutex::new(HubState::default())),
            admission,
        }
    }

    #[must_use]
    pub fn current_id(&self) -> u64 {
        self.state
            .lock()
            .expect("project event hub lock")
            .current_id
    }

    /// Register a subscriber and immediately send a snapshot invalidation.
    ///
    /// Registration and snapshot creation share the client lock with
    /// publication, so an event may be duplicated at the boundary but cannot
    /// be lost.
    pub fn subscribe(&self) -> Option<ProjectEventStream> {
        let admission_permit = self.admission.try_acquire()?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let client_id = {
            let mut state = self.state.lock().expect("project event hub lock");
            let _ = sender.try_send(ProjectEvent::new(state.current_id).sse_frame());
            let client_id = state.next_client_id;
            state.next_client_id += 1;
            state.clients.insert(client_id, sender);
            client_id
        };
        Some(ProjectEventStream {
            receiver,
            client_id,
            state: Arc::downgrade(&self.state),
            _admission_permit: admission_permit,
        })
    }

    /// Advance the session event ID after status and artifacts are visible.
    pub fn publish(&self) -> ProjectEvent {
        // Allocate and broadcast under one lock so concurrent callers cannot
        // deliver a later event ID before an earlier one.
        let mut state = self.state.lock().expect("project event hub lock");
        state.current_id += 1;
        let event_id = state.current_id;
        let event = ProjectEvent::new(event_id);
        let frame = event.sse_frame();
        state
            .clients
            .retain(|_, sender| match sender.try_send(frame.clone()) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
        event
    }

    #[cfg(test)]
    fn subscriber_count(&self) -> usize {
        self.state
            .lock()
            .expect("project event hub lock")
            .clients
            .len()
    }
}

pub struct ProjectEventStream {
    receiver: Receiver<String>,
    client_id: u64,
    state: Weak<Mutex<HubState>>,
    _admission_permit: EventStreamPermit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectEventStreamPoll {
    Frame(String),
    Timeout,
    Closed,
}

impl ProjectEventStream {
    /// Poll once without occupying an executor thread.
    pub fn try_next_frame(&self) -> ProjectEventStreamPoll {
        match self.receiver.try_recv() {
            Ok(frame) => ProjectEventStreamPoll::Frame(frame),
            Err(TryRecvError::Empty) => ProjectEventStreamPoll::Timeout,
            Err(TryRecvError::Disconnected) => ProjectEventStreamPoll::Closed,
        }
    }

    pub fn next_frame_timeout(&self, timeout: Duration) -> ProjectEventStreamPoll {
        match self.receiver.recv_timeout(timeout) {
            Ok(frame) => ProjectEventStreamPoll::Frame(frame),
            Err(RecvTimeoutError::Timeout) => ProjectEventStreamPoll::Timeout,
            Err(RecvTimeoutError::Disconnected) => ProjectEventStreamPoll::Closed,
        }
    }
}

impl Drop for ProjectEventStream {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state
                .lock()
                .expect("project event hub lock")
                .clients
                .remove(&self.client_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(stream: &ProjectEventStream) -> String {
        match stream.next_frame_timeout(Duration::from_millis(10)) {
            ProjectEventStreamPoll::Frame(frame) => frame,
            other => panic!("expected frame, got {other:?}"),
        }
    }

    fn subscribe(hub: &ProjectEventHub) -> ProjectEventStream {
        hub.subscribe().expect("event stream admission")
    }

    #[test]
    fn a_new_subscriber_immediately_invalidates_to_the_current_snapshot() {
        let hub = ProjectEventHub::default();
        hub.publish();
        hub.publish();

        let frame = next(&subscribe(&hub));
        assert!(frame.starts_with("id: 2\nevent: invalidate\n"), "{frame}");
        assert!(frame.contains(PROJECT_EVENT_PROTOCOL), "{frame}");
        assert!(frame.contains(PROJECT_STATUS_PATH), "{frame}");
    }

    #[test]
    fn publications_are_monotonic_and_reach_every_live_subscriber() {
        let hub = ProjectEventHub::default();
        let first = subscribe(&hub);
        let second = subscribe(&hub);
        let _ = next(&first);
        let _ = next(&second);

        assert_eq!(hub.publish().event_id, 1);
        for stream in [&first, &second] {
            assert!(next(stream).starts_with("id: 1\n"));
        }
        assert_eq!(hub.publish().event_id, 2);
        for stream in [&first, &second] {
            assert!(next(stream).starts_with("id: 2\n"));
        }
    }

    #[test]
    fn a_dropped_subscriber_does_not_block_later_publication() {
        let hub = ProjectEventHub::default();
        drop(subscribe(&hub));
        assert_eq!(hub.subscriber_count(), 0);
        assert_eq!(hub.publish().event_id, 1);
        assert_eq!(hub.current_id(), 1);
        assert_eq!(hub.subscriber_count(), 0);
    }

    #[test]
    fn a_stalled_subscriber_has_one_bounded_invalidation() {
        let hub = ProjectEventHub::default();
        let stream = subscribe(&hub);
        for _ in 0..250 {
            hub.publish();
        }

        // The connection may see an older invalidation, but that event tells
        // it to fetch the authoritative snapshot, now at event ID 250. No
        // per-revision queue was accumulated.
        let frame = next(&stream);
        assert!(frame.starts_with("id: 0\n"));
        assert_eq!(hub.current_id(), 250);
        assert_eq!(
            stream.next_frame_timeout(Duration::from_millis(1)),
            ProjectEventStreamPoll::Timeout
        );
    }

    #[test]
    fn session_admission_is_bounded_and_reusable_after_drop() {
        let admission = Arc::new(EventAdmission::new(2));
        let hub = ProjectEventHub::with_admission(Arc::clone(&admission));
        let first = subscribe(&hub);
        let second = subscribe(&hub);
        assert_eq!(admission.active(), 2);
        assert!(hub.subscribe().is_none());

        drop(first);
        assert_eq!(admission.active(), 1);
        let replacement = subscribe(&hub);
        assert_eq!(admission.active(), 2);

        drop(second);
        drop(replacement);
        assert_eq!(admission.active(), 0);
        assert_eq!(hub.subscriber_count(), 0);
    }
}
