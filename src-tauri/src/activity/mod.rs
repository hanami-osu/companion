pub mod model;

mod detector;

use std::collections::VecDeque;

use detector::PlayDetector;
use model::{PlayObservation, RecentPlay};

pub trait ActivityStore: Send {
    fn push(&mut self, play: RecentPlay);
    fn recent(&self) -> Vec<RecentPlay>;
}

pub struct InMemoryActivityStore {
    capacity: usize,
    entries: VecDeque<RecentPlay>,
}

impl InMemoryActivityStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::with_capacity(capacity),
        }
    }
}

impl ActivityStore for InMemoryActivityStore {
    fn push(&mut self, play: RecentPlay) {
        if self.entries.len() == self.capacity {
            self.entries.pop_back();
        }
        self.entries.push_front(play);
    }

    fn recent(&self) -> Vec<RecentPlay> {
        self.entries.iter().cloned().collect()
    }
}

pub struct ActivityTracker {
    detector: PlayDetector,
    store: Box<dyn ActivityStore>,
}

impl Default for ActivityTracker {
    fn default() -> Self {
        Self {
            detector: PlayDetector::default(),
            store: Box::new(InMemoryActivityStore::new(20)),
        }
    }
}

impl ActivityTracker {
    pub fn observe(&mut self, observation: PlayObservation) -> Vec<RecentPlay> {
        if let Some(play) = self.detector.observe(observation) {
            self.store.push(play);
        }
        self.store.recent()
    }
}
