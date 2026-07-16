use chrono::Utc;

use super::model::{PlayObservation, PlayOutcome, RecentPlay, ResultObservation};
use crate::app_state::{BeatmapSummary, CompanionMod, LivePlay, OsuState};

#[derive(Clone, Debug)]
struct PlayCandidate {
    session: u64,
    beatmap: BeatmapSummary,
    live: LivePlay,
    mods: Vec<CompanionMod>,
    retry_detection_ready: bool,
}

#[derive(Default)]
pub struct PlayDetector {
    next_session: u64,
    active: Option<PlayCandidate>,
    pending_exit: Option<PlayCandidate>,
}

impl PlayDetector {
    pub fn observe(&mut self, observation: PlayObservation) -> Option<RecentPlay> {
        match observation.state {
            OsuState::Gameplay => self.observe_gameplay(observation),
            OsuState::Results => self.observe_results(observation.result),
            _ => self.observe_away(),
        }
    }

    fn observe_gameplay(&mut self, observation: PlayObservation) -> Option<RecentPlay> {
        let (Some(beatmap), Some(live)) = (observation.beatmap, observation.live) else {
            return None;
        };
        let mods = observation.mods;

        if let Some(mut previous) = self.pending_exit.take() {
            let same_beatmap = same_beatmap(&previous.beatmap, &beatmap);
            let restarted =
                previous.retry_detection_ready && attempt_restarted(&previous.live, &live);
            if same_beatmap && !previous.live.failed && !restarted {
                previous.retry_detection_ready |= attempt_advanced(&previous.live, &live);
                previous.beatmap = beatmap;
                previous.live = live;
                previous.mods = mods;
                self.active = Some(previous);
                return None;
            }

            let outcome = interrupted_outcome(&previous, &beatmap);
            self.active = Some(self.new_candidate(beatmap, live, mods));
            return Some(build_recent(&previous, None, outcome));
        }

        if let Some(active) = self.active.as_ref() {
            let same_beatmap = same_beatmap(&active.beatmap, &beatmap);
            let restarted = active.retry_detection_ready && attempt_restarted(&active.live, &live);
            if same_beatmap && !restarted {
                let active = self.active.as_mut().expect("active play");
                active.retry_detection_ready |= attempt_advanced(&active.live, &live);
                active.beatmap = beatmap;
                active.live = live;
                active.mods = mods;
                return None;
            }

            let previous = self.active.take().expect("active play");
            let outcome = if previous.live.failed {
                PlayOutcome::Failed
            } else if same_beatmap {
                PlayOutcome::Retried
            } else {
                PlayOutcome::Quit
            };
            self.active = Some(self.new_candidate(beatmap, live, mods));
            return Some(build_recent(&previous, None, outcome));
        }

        self.active = Some(self.new_candidate(beatmap, live, mods));
        None
    }

    fn observe_results(&mut self, result: Option<ResultObservation>) -> Option<RecentPlay> {
        let active = self.active.take().or_else(|| self.pending_exit.take())?;
        let failed = active.live.failed
            || result
                .as_ref()
                .and_then(|value| value.rank.as_deref())
                .is_some_and(is_failure_rank);
        let outcome = if failed {
            PlayOutcome::Failed
        } else {
            PlayOutcome::Passed
        };

        Some(build_recent(&active, result, outcome))
    }

    fn observe_away(&mut self) -> Option<RecentPlay> {
        if let Some(previous) = self.pending_exit.take() {
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

    fn new_candidate(
        &mut self,
        beatmap: BeatmapSummary,
        live: LivePlay,
        mods: Vec<CompanionMod>,
    ) -> PlayCandidate {
        self.next_session += 1;
        PlayCandidate {
            session: self.next_session,
            beatmap,
            live,
            mods,
            retry_detection_ready: false,
        }
    }
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
    let score_id = result.as_ref().and_then(|value| value.score_id);
    let id = score_id.filter(|value| *value > 0).map_or_else(
        || format!("local-{}", active.session),
        |value| format!("score-{value}"),
    );

    RecentPlay {
        id,
        beatmap: active.beatmap.clone(),
        timestamp: result
            .as_ref()
            .and_then(|value| value.timestamp)
            .unwrap_or_else(Utc::now),
        player_name: result
            .as_ref()
            .and_then(|value| value.player_name.clone())
            .or_else(|| active.live.player_name.clone()),
        score: result
            .as_ref()
            .map_or(active.live.score, |value| value.score),
        accuracy: result
            .as_ref()
            .map_or(active.live.accuracy, |value| value.accuracy),
        combo: result
            .as_ref()
            .map_or(active.live.combo, |value| value.combo),
        misses: result
            .as_ref()
            .map_or(active.live.hits.misses, |value| value.misses),
        mods: result
            .as_ref()
            .map_or_else(|| active.mods.clone(), |value| value.mods.clone()),
        pp: result
            .as_ref()
            .and_then(|value| value.pp)
            .or(active.live.current_pp),
        completion: if outcome == PlayOutcome::Passed {
            1.0
        } else {
            active.live.progress.clamp(0.0, 1.0)
        },
        outcome,
        rank,
    }
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
    match (left.beatmap_id, right.beatmap_id) {
        (Some(left), Some(right)) => left == right,
        _ => {
            left.title == right.title
                && left.difficulty == right.difficulty
                && left.mapper == right.mapper
        }
    }
}

fn attempt_restarted(previous: &LivePlay, current: &LivePlay) -> bool {
    let previous_hits = total_hits(&previous.hits);
    let current_hits = total_hits(&current.hits);

    current_hits < previous_hits
        || (current_hits == 0
            && previous.progress > 0.05
            && current.progress + 0.05 < previous.progress)
}

fn attempt_advanced(previous: &LivePlay, current: &LivePlay) -> bool {
    current.score > previous.score || total_hits(&current.hits) > total_hits(&previous.hits)
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

    fn observation(state: OsuState, failed: bool) -> PlayObservation {
        PlayObservation {
            state,
            beatmap: Some(BeatmapSummary {
                beatmap_id: Some(42),
                title: "A map".into(),
                ruleset: Ruleset::Osu,
                ..BeatmapSummary::default()
            }),
            live: Some(LivePlay {
                player_name: Some("player".into()),
                score: 123,
                accuracy: 98.5,
                combo: 100,
                maximum_combo: Some(200),
                hits: HitCounts {
                    misses: u32::from(failed),
                    ..HitCounts::default()
                },
                current_pp: Some(50.0),
                maximum_pp: Some(100.0),
                progress: 0.8,
                failed,
                rank: Some(if failed { "F" } else { "A" }.into()),
            }),
            mods: vec!["HD".into()],
            result: None,
        }
    }

    #[test]
    fn completed_play_is_emitted_once_on_results() {
        let mut detector = PlayDetector::default();
        assert!(
            detector
                .observe(observation(OsuState::Gameplay, false))
                .is_none()
        );

        let mut results = observation(OsuState::Results, false);
        results.result = Some(ResultObservation {
            score_id: Some(99),
            timestamp: None,
            player_name: Some("player".into()),
            score: 500,
            accuracy: 99.0,
            combo: 190,
            misses: 0,
            mods: vec!["HD".into()],
            pp: Some(88.0),
            rank: Some("S".into()),
        });

        let play = detector.observe(results.clone()).expect("completed play");
        assert_eq!(play.id, "score-99");
        assert_eq!(play.outcome, PlayOutcome::Passed);
        assert_eq!(play.rank.as_deref(), Some("S"));
        assert_eq!(play.completion, 1.0);
        assert!(detector.observe(results).is_none());
    }

    #[test]
    fn failed_play_is_recorded_after_leaving_gameplay() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, true));
        assert!(
            detector
                .observe(observation(OsuState::Menu, true))
                .is_none()
        );
        let failed = detector
            .observe(observation(OsuState::Menu, true))
            .expect("failed play");
        assert_eq!(failed.outcome, PlayOutcome::Failed);
        assert_eq!(failed.rank.as_deref(), Some("F"));
        assert_eq!(failed.completion, 0.8);
    }

    #[test]
    fn quit_play_is_recorded_without_a_rank() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));
        assert!(
            detector
                .observe(observation(OsuState::Menu, false))
                .is_none()
        );
        let quit = detector
            .observe(observation(OsuState::Menu, false))
            .expect("quit play");
        assert_eq!(quit.outcome, PlayOutcome::Quit);
        assert_eq!(quit.rank, None);
        assert_eq!(quit.completion, 0.8);
    }

    #[test]
    fn retry_is_detected_when_gameplay_counters_reset() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));

        let mut advanced = observation(OsuState::Gameplay, false);
        let live = advanced.live.as_mut().expect("live play");
        live.score = 200;
        live.hits.great = 1;
        assert!(detector.observe(advanced).is_none());

        let mut restarted = observation(OsuState::Gameplay, false);
        let live = restarted.live.as_mut().expect("live play");
        live.score = 0;
        live.combo = 0;
        live.progress = 0.0;
        let retry = detector.observe(restarted).expect("retried play");
        assert_eq!(retry.outcome, PlayOutcome::Retried);
        assert_eq!(retry.rank, None);
        assert_eq!(retry.completion, 0.8);
    }

    #[test]
    fn initial_gameplay_counter_reset_is_not_a_retry() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));
        assert!(
            detector
                .observe(observation(OsuState::Gameplay, false))
                .is_none()
        );

        let mut settled = observation(OsuState::Gameplay, false);
        let live = settled.live.as_mut().expect("live play");
        live.score = 0;
        live.combo = 0;
        live.hits = HitCounts::default();
        live.progress = 0.0;

        assert!(detector.observe(settled).is_none());
    }

    #[test]
    fn transient_gameplay_state_during_map_load_is_not_a_retry() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));
        assert!(
            detector
                .observe(observation(OsuState::Menu, false))
                .is_none()
        );
        assert!(
            detector
                .observe(observation(OsuState::Gameplay, false))
                .is_none()
        );
    }

    #[test]
    fn same_beatmap_reentry_is_a_retry_after_observed_play() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));

        let mut advanced = observation(OsuState::Gameplay, false);
        let live = advanced.live.as_mut().expect("live play");
        live.score = 200;
        live.hits.great = 1;
        assert!(detector.observe(advanced).is_none());
        assert!(
            detector
                .observe(observation(OsuState::Menu, false))
                .is_none()
        );

        let retry = detector
            .observe(observation(OsuState::Gameplay, false))
            .expect("retried play");
        assert_eq!(retry.outcome, PlayOutcome::Retried);
    }

    #[test]
    fn gameplay_state_flicker_after_activity_is_not_a_retry_without_a_reset() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));

        let mut advanced = observation(OsuState::Gameplay, false);
        advanced.live.as_mut().expect("live play").score = 200;
        assert!(detector.observe(advanced.clone()).is_none());
        assert!(
            detector
                .observe(observation(OsuState::Menu, false))
                .is_none()
        );
        assert!(detector.observe(advanced).is_none());
    }

    #[test]
    fn lazer_score_decreases_are_not_retries_while_hits_advance() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));

        let mut first = observation(OsuState::Gameplay, false);
        let live = first.live.as_mut().expect("live play");
        live.score = 4_771;
        live.hits.great = 22;
        live.progress = 0.08;
        assert!(detector.observe(first).is_none());

        let mut second = observation(OsuState::Gameplay, false);
        let live = second.live.as_mut().expect("live play");
        live.score = 4_738;
        live.hits.great = 24;
        live.progress = 0.09;
        assert!(detector.observe(second).is_none());

        let mut third = observation(OsuState::Gameplay, false);
        let live = third.live.as_mut().expect("live play");
        live.score = 4_461;
        live.hits.great = 26;
        live.progress = 0.10;
        assert!(detector.observe(third).is_none());
    }

    #[test]
    fn a_new_gameplay_session_can_emit_after_results() {
        let mut detector = PlayDetector::default();
        detector.observe(observation(OsuState::Gameplay, false));
        assert!(
            detector
                .observe(observation(OsuState::Results, false))
                .is_some()
        );
        detector.observe(observation(OsuState::Menu, false));
        detector.observe(observation(OsuState::Gameplay, false));
        assert!(
            detector
                .observe(observation(OsuState::Results, false))
                .is_some()
        );
    }
}
