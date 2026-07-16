use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::app_state::{BeatmapSummary, CompanionMod, LivePlay, OsuState};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayOutcome {
    Passed,
    Failed,
    Retried,
    Quit,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentPlay {
    pub id: String,
    pub beatmap: BeatmapSummary,
    pub timestamp: DateTime<Utc>,
    pub player_name: Option<String>,
    pub score: u64,
    pub accuracy: f64,
    pub combo: u32,
    pub misses: u32,
    pub mods: Vec<CompanionMod>,
    pub pp: Option<f64>,
    pub completion: f64,
    pub outcome: PlayOutcome,
    pub rank: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ResultObservation {
    pub score_id: Option<u64>,
    pub timestamp: Option<DateTime<Utc>>,
    pub player_name: Option<String>,
    pub score: u64,
    pub accuracy: f64,
    pub combo: u32,
    pub misses: u32,
    pub mods: Vec<CompanionMod>,
    pub pp: Option<f64>,
    pub rank: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PlayObservation {
    pub state: OsuState,
    pub beatmap: Option<BeatmapSummary>,
    pub live: Option<LivePlay>,
    pub mods: Vec<CompanionMod>,
    pub result: Option<ResultObservation>,
}
