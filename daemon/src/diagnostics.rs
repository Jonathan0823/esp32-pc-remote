use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, time::Instant};

pub const SCHEMA_VERSION: u16 = 1;
pub const MAX_BATCH_EVENTS: usize = 16;
pub const MAX_RECORDS: usize = 128;
pub const MAX_BATCH_BYTES: usize = 4096;
pub const MAX_STORE_BYTES: usize = 32 * 1024;
pub const MAX_STORE_RESPONSE_BYTES: usize = MAX_STORE_BYTES + MAX_RECORDS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    ClockState,
    PreflightRejected,
    HttpResponse,
    TlsTransport,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Ready,
    NotReady,
    WifiUnavailable,
    ClockUnavailable,
    Stale,
    QueueFull,
    Http2xx,
    Http4xx,
    Http5xx,
    HttpOther,
    Dns,
    Connect,
    Certificate,
    Handshake,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub schema: u16,
    pub uptime_ms: u32,
    pub unix_time_ms: Option<u64>,
    pub kind: EventKind,
    pub category: Category,
}

impl Event {
    pub fn valid(&self) -> bool {
        self.schema == SCHEMA_VERSION
            && match self.kind {
                EventKind::ClockState => {
                    matches!(self.category, Category::Ready | Category::NotReady)
                }
                EventKind::PreflightRejected => matches!(
                    self.category,
                    Category::WifiUnavailable
                        | Category::ClockUnavailable
                        | Category::Stale
                        | Category::QueueFull
                ),
                EventKind::HttpResponse => matches!(
                    self.category,
                    Category::Http2xx | Category::Http4xx | Category::Http5xx | Category::HttpOther
                ),
                EventKind::TlsTransport => matches!(
                    self.category,
                    Category::Dns
                        | Category::Connect
                        | Category::Certificate
                        | Category::Handshake
                        | Category::Unknown
                ),
            }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub schema: u16,
    pub events: Vec<Event>,
}

#[derive(Default)]
pub struct Store {
    records: VecDeque<Event>,
    bytes: usize,
    window: Option<Instant>,
    batches_in_window: u8,
}

impl Store {
    pub fn insert(&mut self, batch: Batch) -> Result<(), &'static str> {
        self.insert_at(batch, Instant::now())
    }

    fn insert_at(&mut self, batch: Batch, now: Instant) -> Result<(), &'static str> {
        if self
            .window
            .is_none_or(|started| now.duration_since(started).as_secs() >= 1)
        {
            self.window = Some(now);
            self.batches_in_window = 0;
        }
        if self.batches_in_window >= 4 {
            return Err("rate_limited");
        }
        if batch.schema != SCHEMA_VERSION
            || batch.events.is_empty()
            || batch.events.len() > MAX_BATCH_EVENTS
        {
            return Err("invalid_batch");
        }
        if batch.events.iter().any(|event| !event.valid()) {
            return Err("invalid_event");
        }
        let encoded: Vec<Vec<u8>> = batch
            .events
            .iter()
            .map(serde_json::to_vec)
            .collect::<Result<_, _>>()
            .map_err(|_| "invalid_event")?;
        let incoming: usize = encoded.iter().map(Vec::len).sum();
        if incoming > MAX_BATCH_BYTES {
            return Err("too_large");
        }
        self.batches_in_window += 1;
        for (event, size) in batch
            .events
            .into_iter()
            .zip(encoded.into_iter().map(|v| v.len()))
        {
            while self.records.len() >= MAX_RECORDS || self.bytes + size > MAX_STORE_BYTES {
                if let Some(oldest) = self.records.pop_front() {
                    self.bytes = self
                        .bytes
                        .saturating_sub(serde_json::to_vec(&oldest).map_or(0, |v| v.len()));
                } else {
                    break;
                }
            }
            self.bytes += size;
            self.records.push_back(event);
        }
        Ok(())
    }

    pub fn records(&self) -> Vec<Event> {
        self.records.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(uptime_ms: u32) -> Event {
        Event {
            schema: 1,
            uptime_ms,
            unix_time_ms: None,
            kind: EventKind::ClockState,
            category: Category::NotReady,
        }
    }
    #[test]
    fn accepts_only_allowlisted_typed_records_and_bounded_batches() {
        let raw = br#"{"schema":1,"events":[{"schema":1,"uptime_ms":12,"unix_time_ms":null,"kind":"clock_state","category":"not_ready"}]}"#;
        let batch: Batch = serde_json::from_slice(raw).unwrap();
        assert!(batch.events[0].valid());
        assert!(serde_json::from_slice::<Batch>(br#"{"schema":1,"events":[{"schema":1,"uptime_ms":12,"unix_time_ms":null,"kind":"clock_state","category":"not_ready","message":"secret"}]}"#).is_err());
        let mut store = Store::default();
        store
            .insert(Batch {
                schema: 1,
                events: (0..MAX_BATCH_EVENTS + 1).map(|i| event(i as u32)).collect(),
            })
            .unwrap_err();
    }
    #[test]
    fn store_drops_oldest_at_retention_limit() {
        let mut store = Store::default();
        let start = Instant::now();
        for batch_index in 0..10 {
            let offset = batch_index * MAX_BATCH_EVENTS;
            store
                .insert_at(
                    Batch {
                        schema: 1,
                        events: (offset..offset + MAX_BATCH_EVENTS)
                            .map(|i| event(i as u32))
                            .collect(),
                    },
                    start + std::time::Duration::from_secs((batch_index / 4) as u64),
                )
                .unwrap();
        }
        let records = store.records();
        assert_eq!(records.len(), MAX_RECORDS);
        assert_eq!(records[0].uptime_ms, 32);
    }
    #[test]
    fn rate_limit_caps_accepted_batches_and_recovers_after_window() {
        let mut store = Store::default();
        let start = Instant::now();
        let batch = || Batch {
            schema: 1,
            events: vec![event(1)],
        };
        for _ in 0..4 {
            store.insert_at(batch(), start).unwrap();
        }
        assert_eq!(store.insert_at(batch(), start), Err("rate_limited"));
        assert!(
            store
                .insert_at(batch(), start + std::time::Duration::from_secs(1))
                .is_ok()
        );
    }

    #[test]
    fn categories_must_match_event_kind() {
        assert!(
            !Event {
                schema: 1,
                uptime_ms: 0,
                unix_time_ms: None,
                kind: EventKind::ClockState,
                category: Category::Certificate
            }
            .valid()
        );
    }
}
