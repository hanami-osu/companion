use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::{
    activity::model::{PlayObservation, ResultObservation},
    app_state::{
        BeatmapSummary, CompanionMod, HitCounts, LivePlay, NowPlaying, OsuState, OsuStatus, Ruleset,
    },
};

pub const BACKGROUND_URL: &str = "http://127.0.0.1:24050/files/beatmap/background";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PayloadError {
    #[error("tosu is not ready")]
    NotReady,
    #[error("payload is not valid v2 tosu state")]
    Unexpected,
    #[error("payload could not be decoded: {0}")]
    InvalidJson(String),
}

#[derive(Clone, Debug)]
pub struct NormalizedTosu {
    pub osu: OsuStatus,
    pub now_playing: Option<NowPlaying>,
    pub live_play: Option<LivePlay>,
    pub observation: PlayObservation,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawTosuPayload {
    pub error: Option<String>,
    pub client: Option<String>,
    pub state: RawNumberName,
    pub profile: RawProfile,
    pub beatmap: RawBeatmap,
    pub play: RawPlay,
    pub results_screen: RawResults,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawNumberName {
    pub number: Option<i32>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawProfile {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawBeatmap {
    pub id: Option<i64>,
    pub set: Option<i64>,
    pub checksum: Option<String>,
    pub mode: RawNumberName,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub mapper: Option<String>,
    pub version: Option<String>,
    pub time: RawBeatmapTime,
    pub stats: RawBeatmapStats,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawBeatmapTime {
    pub live: Option<f64>,
    pub first_object: Option<f64>,
    pub last_object: Option<f64>,
    pub mp3_length: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawBeatmapStats {
    pub max_combo: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawPlay {
    pub failed: Option<bool>,
    pub player_name: Option<String>,
    pub mode: RawNumberName,
    pub score: Option<u64>,
    pub accuracy: Option<f64>,
    pub hits: RawHits,
    pub combo: RawCombo,
    pub mods: RawMods,
    pub rank: RawRank,
    pub pp: RawPp,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawHits {
    #[serde(rename = "300")]
    pub great: Option<u32>,
    #[serde(rename = "100")]
    pub ok: Option<u32>,
    #[serde(rename = "50")]
    pub meh: Option<u32>,
    #[serde(rename = "0")]
    pub misses: Option<u32>,
    pub geki: Option<u32>,
    pub katu: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawCombo {
    pub current: Option<u32>,
    pub max: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawMods {
    pub name: Option<String>,
    pub array: Vec<Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawRank {
    pub current: Option<String>,
    pub max_this_play: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawPp {
    pub current: Option<f64>,
    pub fc: Option<f64>,
    pub max_achieved: Option<f64>,
    pub max_achievable: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RawResults {
    pub score_id: Option<i64>,
    pub player_name: Option<String>,
    pub score: Option<u64>,
    pub accuracy: Option<f64>,
    pub hits: RawHits,
    pub mods: RawMods,
    pub max_combo: Option<u32>,
    pub rank: Option<String>,
    pub pp: RawResultPp,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawResultPp {
    pub current: Option<f64>,
    pub fc: Option<f64>,
}

impl RawTosuPayload {
    pub fn parse(payload: &str) -> Result<Self, PayloadError> {
        let value: Self = serde_json::from_str(payload)
            .map_err(|error| PayloadError::InvalidJson(error.to_string()))?;
        if value.error.is_some() {
            return Err(PayloadError::NotReady);
        }
        if value.state.number.is_none()
            || (value.client.is_none()
                && value.beatmap.id.is_none()
                && value.beatmap.title.is_none()
                && value.profile.name.is_none()
                && value.play.score.is_none())
        {
            return Err(PayloadError::Unexpected);
        }
        Ok(value)
    }

    pub fn normalize(self) -> NormalizedTosu {
        let state = normalize_state(self.state.number, self.state.name.as_deref());
        let ruleset = normalize_ruleset(
            self.play.mode.number.or(self.beatmap.mode.number),
            self.play
                .mode
                .name
                .as_deref()
                .or(self.beatmap.mode.name.as_deref()),
        );
        let beatmap = normalize_beatmap(&self.beatmap, ruleset);
        let mods = normalize_mods(&self.play.mods);
        let live_play = (state == OsuState::Gameplay).then(|| normalize_live(&self));
        let result = (state == OsuState::Results).then(|| normalize_result(&self.results_screen));
        let now_playing = beatmap.clone().map(|beatmap| NowPlaying {
            beatmap,
            mods: mods.clone(),
        });

        NormalizedTosu {
            osu: OsuStatus {
                running: true,
                state: state.clone(),
                client: self.client,
            },
            now_playing,
            live_play: live_play.clone(),
            observation: PlayObservation {
                state,
                beatmap,
                live: live_play,
                mods,
                result,
            },
        }
    }
}

fn normalize_state(number: Option<i32>, name: Option<&str>) -> OsuState {
    match number {
        Some(0) => OsuState::Menu,
        Some(2) => OsuState::Gameplay,
        Some(5) => OsuState::SongSelect,
        Some(7) => OsuState::Results,
        _ => match name.unwrap_or_default().to_ascii_lowercase().as_str() {
            "menu" => OsuState::Menu,
            "play" => OsuState::Gameplay,
            "selectplay" => OsuState::SongSelect,
            "resultscreen" => OsuState::Results,
            _ => OsuState::Unknown,
        },
    }
}

fn normalize_ruleset(number: Option<i32>, name: Option<&str>) -> Ruleset {
    match number {
        Some(0) => Ruleset::Osu,
        Some(1) => Ruleset::Taiko,
        Some(2) => Ruleset::Catch,
        Some(3) => Ruleset::Mania,
        _ => match name.unwrap_or_default().to_ascii_lowercase().as_str() {
            "osu" => Ruleset::Osu,
            "taiko" => Ruleset::Taiko,
            "fruits" | "catch" => Ruleset::Catch,
            "mania" => Ruleset::Mania,
            _ => Ruleset::Unknown,
        },
    }
}

fn normalize_beatmap(raw: &RawBeatmap, ruleset: Ruleset) -> Option<BeatmapSummary> {
    if positive_id(raw.id).is_none() && raw.title.as_deref().unwrap_or_default().is_empty() {
        return None;
    }
    Some(BeatmapSummary {
        beatmap_id: positive_id(raw.id),
        beatmap_set_id: positive_id(raw.set),
        artist: raw.artist.clone().unwrap_or_default(),
        title: raw.title.clone().unwrap_or_default(),
        difficulty: raw.version.clone().unwrap_or_default(),
        mapper: raw.mapper.clone().unwrap_or_default(),
        ruleset,
        background_url: background_url(raw),
        max_combo: raw.stats.max_combo,
    })
}

fn background_url(raw: &RawBeatmap) -> Option<String> {
    let cache_key = raw
        .checksum
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| positive_id(raw.id).map(|value| value.to_string()))
        .or_else(|| {
            let title = raw.title.as_deref()?.trim();
            if title.is_empty() {
                return None;
            }
            Some(format!(
                "{title}:{}",
                raw.version.as_deref().unwrap_or_default().trim()
            ))
        })?;

    let mut url = Url::parse(BACKGROUND_URL).expect("background endpoint must be a valid URL");
    url.query_pairs_mut().append_pair("beatmap", &cache_key);
    Some(url.to_string())
}

fn normalize_live(raw: &RawTosuPayload) -> LivePlay {
    let start = raw.beatmap.time.first_object.unwrap_or(0.0);
    let end = raw
        .beatmap
        .time
        .last_object
        .or(raw.beatmap.time.mp3_length)
        .unwrap_or(start);
    let progress = if end > start {
        ((raw.beatmap.time.live.unwrap_or(start) - start) / (end - start)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    LivePlay {
        player_name: raw
            .play
            .player_name
            .clone()
            .or_else(|| raw.profile.name.clone())
            .filter(|value| !value.is_empty()),
        score: raw.play.score.unwrap_or_default(),
        accuracy: raw.play.accuracy.unwrap_or_default(),
        combo: raw.play.combo.current.unwrap_or_default(),
        maximum_combo: raw.beatmap.stats.max_combo.or(raw.play.combo.max),
        hits: normalize_hits(&raw.play.hits),
        current_pp: raw.play.pp.current,
        maximum_pp: raw
            .play
            .pp
            .max_achievable
            .or(raw.play.pp.max_achieved)
            .or(raw.play.pp.fc),
        progress,
        failed: raw.play.failed.unwrap_or(false),
        rank: raw.play.rank.current.clone(),
    }
}

fn normalize_result(raw: &RawResults) -> ResultObservation {
    ResultObservation {
        score_id: positive_id(raw.score_id),
        timestamp: raw.created_at.as_deref().and_then(parse_timestamp),
        player_name: raw.player_name.clone().filter(|value| !value.is_empty()),
        score: raw.score.unwrap_or_default(),
        accuracy: raw.accuracy.unwrap_or_default(),
        combo: raw.max_combo.unwrap_or_default(),
        misses: raw.hits.misses.unwrap_or_default(),
        mods: normalize_mods(&raw.mods),
        pp: raw.pp.current,
        rank: raw.rank.clone().filter(|value| !value.is_empty()),
    }
}

fn positive_id(value: Option<i64>) -> Option<u64> {
    value
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn normalize_hits(raw: &RawHits) -> HitCounts {
    HitCounts {
        great: raw.great.unwrap_or_default(),
        ok: raw.ok.unwrap_or_default(),
        meh: raw.meh.unwrap_or_default(),
        katu: raw.katu.unwrap_or_default(),
        geki: raw.geki.unwrap_or_default(),
        misses: raw.misses.unwrap_or_default(),
    }
}

fn normalize_mods(raw: &RawMods) -> Vec<CompanionMod> {
    let mut mods = Vec::new();
    for value in &raw.array {
        let acronym = value
            .as_str()
            .or_else(|| value.get("acronym").and_then(Value::as_str));
        if let Some(acronym) = acronym {
            let settings = value
                .get("settings")
                .and_then(Value::as_object)
                .map(|settings| {
                    settings
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default();
            push_mod(&mut mods, acronym, settings);
        }
    }

    if mods.is_empty()
        && let Some(name) = raw.name.as_deref()
    {
        let compact = name.trim();
        if compact.is_ascii() && compact.len().is_multiple_of(2) {
            for acronym in compact.as_bytes().chunks_exact(2) {
                if let Ok(acronym) = std::str::from_utf8(acronym) {
                    push_mod(&mut mods, acronym, Default::default());
                }
            }
        } else {
            push_mod(&mut mods, compact, Default::default());
        }
    }

    mods
}

fn push_mod(
    mods: &mut Vec<CompanionMod>,
    acronym: &str,
    settings: std::collections::BTreeMap<String, Value>,
) {
    let acronym = acronym.trim().to_ascii_uppercase();
    if acronym.is_empty() || acronym == "NM" {
        return;
    }
    if let Some(existing) = mods.iter_mut().find(|item| item.acronym == acronym) {
        existing.settings.extend(settings);
        return;
    }
    mods.push(CompanionMod { acronym, settings });
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_representative_v2_gameplay_fixture() {
        let raw = RawTosuPayload::parse(include_str!("fixtures/v2_gameplay.json"))
            .expect("valid fixture");
        let normalized = raw.normalize();
        assert_eq!(normalized.osu.state, OsuState::Gameplay);
        let now_playing = normalized.now_playing.expect("beatmap");
        assert_eq!(now_playing.beatmap.beatmap_id, Some(1234));
        assert_eq!(
            now_playing.beatmap.background_url.as_deref(),
            Some("http://127.0.0.1:24050/files/beatmap/background?beatmap=fixture-checksum")
        );
        assert_eq!(mod_acronyms(&now_playing.mods), ["HD", "DT"]);
        assert_eq!(
            now_playing.mods[1].settings.get("speed_change"),
            Some(&serde_json::json!(1.5))
        );
        let live = normalized.live_play.expect("live play");
        assert_eq!(live.hits.misses, 1);
        assert!((live.progress - 0.5).abs() < 0.001);
    }

    #[test]
    fn accepts_unknown_and_missing_optional_fields() {
        let raw = RawTosuPayload::parse(
            r#"{"client":"future","state":{"number":999,"name":"newState"},"unknown":{"nested":true}}"#,
        )
        .expect("tolerant payload");
        let normalized = raw.normalize();
        assert_eq!(normalized.osu.state, OsuState::Unknown);
        assert!(normalized.now_playing.is_none());
    }

    #[test]
    fn exposes_selected_lazer_mods_before_gameplay_without_a_whitelist() {
        let raw = RawTosuPayload::parse(
            r#"{
                "client":"lazer",
                "state":{"number":5,"name":"selectPlay"},
                "beatmap":{"id":1234,"title":"Selected map"},
                "play":{"mods":{"array":[
                    {"acronym":"DA","settings":{"circle_size":6.5}},
                    {"acronym":"WG"},
                    {"acronym":"SV2"},
                    {"acronym":"future_mod"},
                    {"acronym":"DA"}
                ]}}
            }"#,
        )
        .expect("song select payload");

        let normalized = raw.normalize();
        assert_eq!(normalized.osu.state, OsuState::SongSelect);
        let mods = normalized.now_playing.expect("selected map").mods;
        assert_eq!(mod_acronyms(&mods), ["DA", "WG", "SV2", "FUTURE_MOD"]);
        assert_eq!(
            mods[0].settings.get("circle_size"),
            Some(&serde_json::json!(6.5))
        );
    }

    #[test]
    fn splits_compact_stable_mod_name_when_the_array_is_missing() {
        let mods = normalize_mods(&RawMods {
            name: Some("HDDT".into()),
            array: Vec::new(),
        });

        assert_eq!(mod_acronyms(&mods), ["HD", "DT"]);
    }

    fn mod_acronyms(mods: &[CompanionMod]) -> Vec<&str> {
        mods.iter()
            .map(|game_mod| game_mod.acronym.as_str())
            .collect()
    }

    #[test]
    fn arbitrary_json_is_not_a_healthy_v2_payload() {
        assert!(matches!(
            RawTosuPayload::parse(r#"{"hello":"world"}"#),
            Err(PayloadError::Unexpected)
        ));
    }

    #[test]
    fn accepts_negative_sentinel_ids_for_local_beatmaps() {
        let raw = RawTosuPayload::parse(
            r#"{
                "client":"lazer",
                "state":{"number":7,"name":"resultScreen"},
                "beatmap":{
                    "id":-1,
                    "set":-1,
                    "checksum":"local-checksum",
                    "title":"Local map",
                    "version":"Test"
                },
                "resultsScreen":{"scoreId":-1,"score":12345}
            }"#,
        )
        .expect("local beatmap payload");

        let normalized = raw.normalize();
        let beatmap = normalized.now_playing.expect("local beatmap").beatmap;
        assert_eq!(beatmap.beatmap_id, None);
        assert_eq!(beatmap.beatmap_set_id, None);
        assert_eq!(
            beatmap.background_url.as_deref(),
            Some("http://127.0.0.1:24050/files/beatmap/background?beatmap=local-checksum")
        );
        assert_eq!(
            normalized.observation.result.expect("result").score_id,
            None
        );
    }

    #[test]
    fn background_url_changes_with_the_beatmap() {
        let first = RawBeatmap {
            id: Some(1234),
            ..RawBeatmap::default()
        };
        let second = RawBeatmap {
            id: Some(5678),
            ..RawBeatmap::default()
        };

        assert_ne!(background_url(&first), background_url(&second));
    }

    #[test]
    fn background_url_encodes_metadata_fallback() {
        let beatmap = RawBeatmap {
            title: Some("A title & more".to_owned()),
            version: Some("Hard+".to_owned()),
            ..RawBeatmap::default()
        };

        assert_eq!(
            background_url(&beatmap).as_deref(),
            Some(
                "http://127.0.0.1:24050/files/beatmap/background?beatmap=A+title+%26+more%3AHard%2B"
            )
        );
    }
}
