//! Event generation and management

use std::collections::BTreeMap;

/// Event data types
#[derive(Debug, Clone)]
pub enum EventData {
    /// Chip-specific event
    Chip(ChipEvent),
    /// Raw VGM command byte
    Raw(u8),
}

/// Chip-specific event data
#[derive(Debug, Clone)]
pub struct ChipEvent {
    /// Event type (chip-specific)
    pub event_type: u16,
    /// Primary value
    pub value1: i32,
    /// Secondary value
    pub value2: i32,
}

impl ChipEvent {
    pub fn new(event_type: u16, value1: i32, value2: i32) -> Self {
        Self {
            event_type,
            value1,
            value2,
        }
    }
}

/// Event with timing and channel info
#[derive(Debug, Clone)]
pub struct Event {
    /// Time in samples
    pub time: i64,
    /// Channel index (-1 for global/raw)
    pub channel: i8,
    /// Event data
    pub data: EventData,
}

impl Event {
    pub fn new(time: i64, channel: i8, data: EventData) -> Self {
        Self {
            time,
            channel,
            data,
        }
    }

    pub fn chip(time: i64, channel: i8, event_type: u16, value1: i32, value2: i32) -> Self {
        Self::new(
            time,
            channel,
            EventData::Chip(ChipEvent::new(event_type, value1, value2)),
        )
    }

    pub fn raw(time: i64, value: u8) -> Self {
        Self::new(time, -1, EventData::Raw(value))
    }
}

/// Time-sorted event queue
#[derive(Debug, Default)]
pub struct EventQueue {
    /// Events grouped by time
    events: BTreeMap<i64, Vec<Event>>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an event into the queue
    pub fn insert(&mut self, event: Event) {
        self.events
            .entry(event.time)
            .or_default()
            .push(event);
    }

    /// Get all events in time order
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        self.events.values().flatten()
    }

    /// Get events at a specific time
    pub fn at_time(&self, time: i64) -> Option<&Vec<Event>> {
        self.events.get(&time)
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get the last event time
    pub fn last_time(&self) -> Option<i64> {
        self.events.keys().next_back().copied()
    }
}
