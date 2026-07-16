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

#[derive(Clone, Debug, PartialEq)]
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
        merge_meaningful(&mut self.score, newer.score, |value| *value > 0);
        merge_meaningful(&mut self.accuracy, newer.accuracy, |value| *value > 0.0);
        merge_meaningful(&mut self.combo, newer.combo, |value| *value > 0);
        self.misses = newer.misses.or(self.misses);
        merge_meaningful(&mut self.judged_objects, newer.judged_objects, |value| {
            *value > 0
        });
        if !newer.mods.is_empty() {
            self.mods = newer.mods;
        }
        merge_meaningful(&mut self.pp, newer.pp, |value| *value > 0.0);
        self.rank = newer.rank.or_else(|| self.rank.take());
    }
}

fn merge_meaningful<T>(
    current: &mut Option<T>,
    newer: Option<T>,
    meaningful: impl FnOnce(&T) -> bool,
) {
    if newer.as_ref().is_some_and(meaningful) || current.is_none() && newer.is_some() {
        *current = newer;
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
