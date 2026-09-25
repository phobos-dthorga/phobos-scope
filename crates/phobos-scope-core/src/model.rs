use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;
pub const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_DEFINITIONS: usize = 256;
pub const MAX_RECORDS: usize = 20_000;
pub const MAX_TEXT_BYTES: usize = 512;
pub const MAX_CAPTURE_SECONDS: u64 = 3_600;
pub const MAX_CLOCK_HZ: u64 = 1_000_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Summary,
    Detailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Operation,
    Gauge,
    Cumulative,
    Increment,
    Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: usize,
    pub name: String,
    pub category: String,
    pub unit: String,
    pub kind: Kind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_records: usize,
    pub max_depth: usize,
    pub max_duration_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aggregate {
    pub metric: usize,
    pub calls: u64,
    pub total_ticks: u64,
    pub max_ticks: u64,
    pub incomplete: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub metric: usize,
    pub start_tick: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub metric: usize,
    pub tick: u64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub metric: usize,
    pub tick: u64,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub format_version: u32,
    pub recorder_version: String,
    pub capture_id: String,
    pub mode: Mode,
    pub clock_frequency_hz: u64,
    pub end_tick: u64,
    pub stop_reason: String,
    pub limits: Limits,
    pub definitions: Vec<Definition>,
    pub aggregates: Vec<Aggregate>,
    pub events: Vec<Event>,
    pub counters: Vec<Sample>,
    pub contexts: Vec<Context>,
    pub metadata: Vec<Metadata>,
    pub dropped_records: u64,
    pub rejected_measurements: u64,
}
