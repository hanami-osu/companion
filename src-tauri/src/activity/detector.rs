use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::model::{PlayObservation, PlayOutcome, RecentPlay, ResultObservation};
use crate::app_state::{BeatmapSummary, CompanionMod, LivePlay, OsuState};

const MIN_ACTIVE_SECONDS: f64 = 5.0;
const MIN_JUDGED_OBJECTS: u64 = 10;
const MIN_PROGRESS: f64 = 0.02;
const RESULT_GRACE_PERIOD: Duration = Duration::from_millis(400);
const RESULT_SETTLE_PERIOD: Duration = Duration::from_millis(100);
const RESULT_HARD_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug)]
struct PlayCandidate {
    id: Uuid,
    started_at: DateTime<Utc>,
    beatmap: BeatmapSummary,
    live: LivePlay,
    mods: Vec<CompanionMod>,
    retry_detection_ready: bool,
}

#[derive(Clone, Debug)]
struct PendingResults {
    candidate: PlayCandidate,
    result: Option<ResultObservation>,
    first_seen_at: Instant,
    last_updated_at: Instant,
}

#[derive(Default)]
pub struct PlayDetector {
    active: Option<PlayCandidate>,
    pending_exit: Option<PlayCandidate>,
    pending_results: Option<PendingResults>,
}

impl PlayDetector {
    pub fn reset(&mut self) {
        self.active = None;
        self.pending_exit = None;
        self.pending_results = None;
    }

    pub fn observe(&mut self, observation: PlayObservation) -> Option<RecentPlay> {
        self.observe_at(observation, Instant::now())
    }

    fn observe_at(&mut self, observation: PlayObservation, now: Instant) -> Option<RecentPlay> {
        match observation.state {
            OsuState::Gameplay => self.observe_gameplay(observation),
            OsuState::Results => self.observe_results(observation.result, now),
            _ => self.observe_away(),
        }
    }

    fn observe_gameplay(&mut self, observation: PlayObservation) -> Option<RecentPlay> {
        let completed_result = self.take_pending_on_exit();
        let (Some(beatmap), Some(live)) = (observation.beatmap, observation.live) else {
            return completed_result;
        };
        let mods = observation.mods;

        if completed_result.is_some() {
            self.active = Some(new_candidate(beatmap, live, mods));
            return completed_result;
        }

        if let Some(mut previous) = self.pending_exit.take() {
            let same = same_beatmap(&previous.beatmap, &beatmap);
            let restarted =
                previous.retry_detection_ready && attempt_restarted(&previous.live, &live);
            if same && !previous.live.failed && !restarted {
                update_candidate(&mut previous, beatmap, live, mods);
                self.active = Some(previous);
                return None;
            }

            let outcome = interrupted_outcome(&previous, &beatmap);
            let recent =
                meaningful_attempt(&previous.live).then(|| build_recent(&previous, None, outcome));
            self.active = Some(new_candidate(beatmap, live, mods));
            return recent;
        }

        if let Some(active) = self.active.as_ref() {
            let same = same_beatmap(&active.beatmap, &beatmap);
            let restarted = active.retry_detection_ready && attempt_restarted(&active.live, &live);
            if same && !restarted {
                update_candidate(
                    self.active.as_mut().expect("active play"),
                    beatmap,
                    live,
                    mods,
                );
                return None;
            }

            let previous = self.active.take().expect("active play");
            let outcome = if previous.live.failed {
                PlayOutcome::Failed
            } else if same {
                PlayOutcome::Retried
            } else {
                PlayOutcome::Quit
            };
            let recent =
                meaningful_attempt(&previous.live).then(|| build_recent(&previous, None, outcome));
            self.active = Some(new_candidate(beatmap, live, mods));
            return recent;
        }

        self.active = Some(new_candidate(beatmap, live, mods));
        None
    }

    fn observe_results(
        &mut self,
        result: Option<ResultObservation>,
        now: Instant,
    ) -> Option<RecentPlay> {
        if let Some(pending) = self.pending_results.as_mut() {
            if merge_result(&mut pending.result, result) {
                pending.last_updated_at = now;
            }
            let age = now.duration_since(pending.first_seen_at);
            let settled_for = now.duration_since(pending.last_updated_at);
            let ready = pending
                .result
                .as_ref()
                .is_some_and(ResultObservation::is_ready);
            if age >= RESULT_HARD_TIMEOUT {
                let pending = self.pending_results.take().expect("pending results");
                return finalize_pending(pending);
            }
            if ready && age >= RESULT_GRACE_PERIOD && settled_for >= RESULT_SETTLE_PERIOD {
                let pending = self.pending_results.take().expect("pending results");
                return finalize_pending(pending);
            }
            return None;
        }

        let candidate = self.active.take().or_else(|| self.pending_exit.take())?;
        if !meaningful_attempt(&candidate.live) {
            return None;
        }
        self.pending_results = Some(PendingResults {
            candidate,
            result,
            first_seen_at: now,
            last_updated_at: now,
        });
        None
    }

    fn observe_away(&mut self) -> Option<RecentPlay> {
        if self.pending_results.is_some() {
            return self.take_pending_on_exit();
        }
        if let Some(previous) = self.pending_exit.take() {
            if !meaningful_attempt(&previous.live) {
                return None;
            }
            let outcome = if previous.live.failed {
                PlayOutcome::Failed
            } else {
                PlayOutcome::Quit
            };
            return Some(build_recent(&previous, None, outcome));
        }

        self.pending_exit = self.active.take();
        None
    }

    fn take_pending_on_exit(&mut self) -> Option<RecentPlay> {
        self.pending_results.take().and_then(finalize_pending)
    }
}

fn new_candidate(
    beatmap: BeatmapSummary,
    live: LivePlay,
    mods: Vec<CompanionMod>,
) -> PlayCandidate {
    PlayCandidate {
        id: Uuid::new_v4(),
        started_at: Utc::now(),
        beatmap,
        live,
        mods,
        retry_detection_ready: false,
    }
}

fn update_candidate(
    candidate: &mut PlayCandidate,
    beatmap: BeatmapSummary,
    live: LivePlay,
    mods: Vec<CompanionMod>,
) {
    candidate.retry_detection_ready |= attempt_advanced(&candidate.live, &live);
    candidate.beatmap = beatmap;
    candidate.live = live;
    candidate.mods = mods;
}

fn merge_result(current: &mut Option<ResultObservation>, newer: Option<ResultObservation>) -> bool {
    let before = current.clone();
    match (current.as_mut(), newer) {
        (Some(current), Some(newer)) => current.merge(newer),
        (None, Some(newer)) => *current = Some(newer),
        _ => {}
    }
    *current != before
}

fn finalize_pending(pending: PendingResults) -> Option<RecentPlay> {
    let failed = pending.candidate.live.failed
        || pending
            .result
            .as_ref()
            .and_then(|value| value.rank.as_deref())
            .is_some_and(is_failure_rank);
    let usable = failed
        || pending
            .result
            .as_ref()
            .is_some_and(ResultObservation::is_ready);
    if !usable {
        return None;
    }
    let outcome = if failed {
        PlayOutcome::Failed
    } else {
        PlayOutcome::Passed
    };
    Some(build_recent(&pending.candidate, pending.result, outcome))
}

fn build_recent(
    active: &PlayCandidate,
    result: Option<ResultObservation>,
    outcome: PlayOutcome,
) -> RecentPlay {
    let observed_rank = result
        .as_ref()
        .and_then(|value| value.rank.clone())
        .or_else(|| active.live.rank.clone());
    let rank = match outcome {
        PlayOutcome::Passed => observed_rank,
        PlayOutcome::Failed => Some("F".into()),
        PlayOutcome::Retried | PlayOutcome::Quit => None,
    };
    let ended_at = result
        .as_ref()
        .and_then(|value| value.timestamp)
        .unwrap_or_else(Utc::now);

    RecentPlay {
        id: active.id.to_string(),
        score_id: result.as_ref().and_then(|value| value.score_id),
        beatmap: active.beatmap.clone(),
        started_at: active.started_at,
        ended_at,
        player_name: result
            .as_ref()
            .and_then(|value| value.player_name.clone())
            .or_else(|| active.live.player_name.clone()),
        score: result
            .as_ref()
            .and_then(|value| value.score)
            .unwrap_or(active.live.score),
        accuracy: result
            .as_ref()
            .and_then(|value| value.accuracy)
            .unwrap_or(active.live.accuracy),
        combo: result
            .as_ref()
            .and_then(|value| value.combo)
            .unwrap_or(active.live.combo),
        misses: result
            .as_ref()
            .and_then(|value| value.misses)
            .unwrap_or(active.live.hits.misses),
        mods: result.as_ref().map_or_else(
            || active.mods.clone(),
            |value| {
                if value.mods.is_empty() {
                    active.mods.clone()
                } else {
                    value.mods.clone()
                }
            },
        ),
        pp: result
            .as_ref()
            .and_then(|value| value.pp)
            .or(active.live.current_pp),
        completion: (outcome != PlayOutcome::Passed)
            .then_some(active.live.progress.clamp(0.0, 1.0)),
        outcome,
        rank,
    }
}

fn meaningful_attempt(live: &LivePlay) -> bool {
    live.elapsed_seconds >= MIN_ACTIVE_SECONDS
        || total_hits(&live.hits) >= MIN_JUDGED_OBJECTS
        || live.progress >= MIN_PROGRESS
}

fn interrupted_outcome(previous: &PlayCandidate, next_beatmap: &BeatmapSummary) -> PlayOutcome {
    if previous.live.failed {
        PlayOutcome::Failed
    } else if same_beatmap(&previous.beatmap, next_beatmap) {
        PlayOutcome::Retried
    } else {
        PlayOutcome::Quit
    }
}

fn same_beatmap(left: &BeatmapSummary, right: &BeatmapSummary) -> bool {
    if let (Some(left), Some(right)) = (
        left.checksum
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        right
            .checksum
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        return left.eq_ignore_ascii_case(right);
    }
    match (left.beatmap_id, right.beatmap_id) {
        (Some(left), Some(right)) if left > 0 && right > 0 => left == right,
        _ => {
            left.artist == right.artist
                && left.title == right.title
                && left.difficulty == right.difficulty
                && left.mapper == right.mapper
        }
    }
}

fn attempt_restarted(previous: &LivePlay, current: &LivePlay) -> bool {
    let previous_hits = total_hits(&previous.hits);
    let current_hits = total_hits(&current.hits);
    current_hits < previous_hits
        || current.elapsed_seconds + 1.0 < previous.elapsed_seconds
        || (current_hits == 0
            && previous.progress > 0.05
            && current.progress + 0.05 < previous.progress)
}

fn attempt_advanced(previous: &LivePlay, current: &LivePlay) -> bool {
    total_hits(&current.hits) > total_hits(&previous.hits)
        || current.elapsed_seconds > previous.elapsed_seconds + 0.5
        || current.progress > previous.progress + 0.005
}

fn total_hits(hits: &crate::app_state::HitCounts) -> u64 {
    u64::from(hits.great)
        + u64::from(hits.ok)
        + u64::from(hits.meh)
        + u64::from(hits.katu)
        + u64::from(hits.geki)
        + u64::from(hits.misses)
}

fn is_failure_rank(rank: &str) -> bool {
    rank.eq_ignore_ascii_case("F")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{HitCounts, Ruleset};

    fn observation(state: OsuState, progress: f64) -> PlayObservation {
        PlayObservation {
            state,
            beatmap: Some(BeatmapSummary {
                beatmap_id: Some(42),
                checksum: Some("checksum-a".into()),
                title: "A map".into(),
                ruleset: Ruleset::Osu,
                ..BeatmapSummary::default()
            }),
            live: Some(LivePlay {
                player_name: Some("player".into()),
                score: 123,
                accuracy: 98.5,
                combo: 20,
                maximum_combo: Some(200),
                hits: HitCounts::default(),
                current_pp: Some(50.0),
                maximum_pp: Some(100.0),
                progress,
                elapsed_seconds: progress * 100.0,
                failed: false,
                rank: Some("A".into()),
            }),
            mods: vec!["HD".into()],
            result: None,
        }
    }

    fn ready_result() -> ResultObservation {
        ResultObservation {
            score_id: Some(99),
            timestamp: None,
            player_name: Some("player".into()),
            score: Some(500),
            accuracy: Some(99.0),
            combo: Some(190),
            misses: Some(0),
            judged_objects: Some(200),
            mods: vec!["HD".into()],
            pp: Some(88.0),
            rank: Some("S".into()),
        }
    }

    #[test]
    fn reset_discards_an_attempt_across_an_observation_gap() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, 0.5));
        detector.reset();
        assert!(
            detector
                .observe(observation(OsuState::Gameplay, 0.0))
                .is_none()
        );
        assert!(detector.observe(observation(OsuState::Menu, 0.5)).is_none());
        assert!(detector.observe(observation(OsuState::Menu, 0.5)).is_none());
    }

    #[test]
    fn empty_attempt_is_discarded() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, 0.0));
        detector.observe(observation(OsuState::Menu, 0.0));
        assert!(detector.observe(observation(OsuState::Menu, 0.0)).is_none());
    }

    #[test]
    fn meaningful_quit_is_recorded() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, 0.2));
        detector.observe(observation(OsuState::Menu, 0.2));
        let play = detector
            .observe(observation(OsuState::Menu, 0.2))
            .expect("quit");
        assert_eq!(play.outcome, PlayOutcome::Quit);
        assert_eq!(play.completion, Some(0.2));
        assert!(Uuid::parse_str(&play.id).is_ok());
    }

    #[test]
    fn meaningful_retry_is_recorded() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, 0.2));
        let mut advanced = observation(OsuState::Gameplay, 0.3);
        advanced.live.as_mut().expect("live").hits.great = 20;
        detector.observe(advanced);
        let retry = detector
            .observe(observation(OsuState::Gameplay, 0.0))
            .expect("retry");
        assert_eq!(retry.outcome, PlayOutcome::Retried);
    }

    #[test]
    fn ready_result_collects_late_pp_and_finalizes_once_after_grace_period() {
        let mut detector = PlayDetector::default();
        let started = Instant::now();
        detector.observe_at(observation(OsuState::Gameplay, 0.9), started);
        let mut first = observation(OsuState::Results, 0.9);
        let mut initial_result = ready_result();
        initial_result.pp = None;
        first.result = Some(initial_result);
        assert!(detector.observe_at(first, started).is_none());

        let mut complete = observation(OsuState::Results, 0.9);
        complete.result = Some(ready_result());
        assert!(
            detector
                .observe_at(complete.clone(), started + Duration::from_millis(250))
                .is_none()
        );
        assert!(
            detector
                .observe_at(complete.clone(), started + Duration::from_millis(399))
                .is_none()
        );
        let play = detector
            .observe_at(complete.clone(), started + RESULT_GRACE_PERIOD)
            .expect("complete result");
        assert_eq!(play.score_id, Some(99));
        assert_eq!(play.pp, Some(88.0));
        assert_eq!(play.completion, None);
        assert!(
            detector
                .observe_at(complete, started + Duration::from_millis(500))
                .is_none()
        );
    }

    #[test]
    fn leaving_results_finalizes_ready_data_and_discards_unusable_data() {
        let started = Instant::now();
        let mut detector = PlayDetector::default();
        detector.observe_at(observation(OsuState::Gameplay, 0.9), started);
        let mut results = observation(OsuState::Results, 0.9);
        results.result = Some(ready_result());
        assert!(detector.observe_at(results, started).is_none());
        let play = detector
            .observe_at(
                observation(OsuState::Menu, 0.9),
                started + Duration::from_millis(50),
            )
            .expect("ready result on exit");
        assert_eq!(play.outcome, PlayOutcome::Passed);

        let mut detector = PlayDetector::default();
        detector.observe_at(observation(OsuState::Gameplay, 0.9), started);
        let mut unusable = observation(OsuState::Results, 0.9);
        unusable.result = Some(ResultObservation {
            score_id: None,
            timestamp: None,
            player_name: None,
            score: Some(0),
            accuracy: Some(0.0),
            combo: Some(0),
            misses: None,
            judged_objects: None,
            mods: Vec::new(),
            pp: None,
            rank: None,
        });
        detector.observe_at(unusable, started);
        assert!(
            detector
                .observe_at(
                    observation(OsuState::Menu, 0.9),
                    started + Duration::from_millis(50),
                )
                .is_none()
        );
    }

    #[test]
    fn hard_timeout_uses_reliable_failed_data_and_discards_unknown_results() {
        let started = Instant::now();
        let mut failed_gameplay = observation(OsuState::Gameplay, 0.9);
        failed_gameplay.live.as_mut().expect("live").failed = true;
        let mut detector = PlayDetector::default();
        detector.observe_at(failed_gameplay, started);
        detector.observe_at(observation(OsuState::Results, 0.9), started);
        let failed = detector
            .observe_at(
                observation(OsuState::Results, 0.9),
                started + RESULT_HARD_TIMEOUT,
            )
            .expect("reliable failure");
        assert_eq!(failed.outcome, PlayOutcome::Failed);

        let mut detector = PlayDetector::default();
        detector.observe_at(observation(OsuState::Gameplay, 0.9), started);
        detector.observe_at(observation(OsuState::Results, 0.9), started);
        assert!(
            detector
                .observe_at(
                    observation(OsuState::Results, 0.9),
                    started + RESULT_HARD_TIMEOUT,
                )
                .is_none()
        );
    }

    #[test]
    fn checksum_has_priority_over_matching_metadata_and_ids() {
        let mut left = observation(OsuState::Gameplay, 0.2)
            .beatmap
            .expect("beatmap");
        let mut right = left.clone();
        right.checksum = Some("other".into());
        assert!(!same_beatmap(&left, &right));
        left.beatmap_id = None;
        right.beatmap_id = None;
        right.checksum = Some("CHECKSUM-A".into());
        assert!(same_beatmap(&left, &right));
    }

    #[test]
    fn meaningful_policy_accepts_each_supported_signal() {
        let mut live = observation(OsuState::Gameplay, 0.0).live.expect("live");
        assert!(!meaningful_attempt(&live));
        live.elapsed_seconds = MIN_ACTIVE_SECONDS;
        assert!(meaningful_attempt(&live));
        live.elapsed_seconds = 0.0;
        live.hits.great = MIN_JUDGED_OBJECTS as u32;
        assert!(meaningful_attempt(&live));
        live.hits = HitCounts::default();
        live.progress = MIN_PROGRESS;
        assert!(meaningful_attempt(&live));
    }
}
