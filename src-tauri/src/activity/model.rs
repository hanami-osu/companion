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
    pub score_id: Option<u64>,
    pub beatmap: BeatmapSummary,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub player_name: Option<String>,
    pub score: u64,
    pub accuracy: f64,
    pub combo: u32,
    pub misses: u32,
    pub mods: Vec<CompanionMod>,
    pub pp: Option<f64>,
    pub completion: Option<f64>,
    pub outcome: PlayOutcome,
    pub rank: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ResultObservation {
    pub score_id: Option<u64>,
    pub timestamp: Option<DateTime<Utc>>,
    pub player_name: Option<String>,
    pub score: Option<u64>,
    pub accuracy: Option<f64>,
    pub combo: Option<u32>,
    pub misses: Option<u32>,
    pub judged_objects: Option<u64>,
    pub mods: Vec<CompanionMod>,
    pub pp: Option<f64>,
    pub rank: Option<String>,
}

impl ResultObservation {
    pub fn is_ready(&self) -> bool {
        let has_identity_or_score =
            self.score_id.is_some() || self.score.is_some_and(|score| score > 0);
        let has_result_detail = self.accuracy.is_some_and(|accuracy| accuracy > 0.0)
            || self.judged_objects.is_some_and(|hits| hits > 0)
            || self.rank.is_some()
            || self.pp.is_some();
        has_identity_or_score && has_result_detail
    }

    pub fn merge(&mut self, newer: Self) {
        self.score_id = newer.score_id.or(self.score_id);
        self.timestamp = newer.timestamp.or(self.timestamp);
        self.player_name = newer.player_name.or_else(|| self.player_name.take());
        self.score = newer.score.or(self.score);
        self.accuracy = newer.accuracy.or(self.accuracy);
        self.combo = newer.combo.or(self.combo);
        self.misses = newer.misses.or(self.misses);
        self.judged_objects = newer.judged_objects.or(self.judged_objects);
        if !newer.mods.is_empty() {
            self.mods = newer.mods;
        }
        self.pp = newer.pp.or(self.pp);
        self.rank = newer.rank.or_else(|| self.rank.take());
    }
}

#[derive(Clone, Debug)]
pub struct PlayObservation {
    pub state: OsuState,
    pub beatmap: Option<BeatmapSummary>,
    pub live: Option<LivePlay>,
    pub mods: Vec<CompanionMod>,
    pub result: Option<ResultObservation>,
}
