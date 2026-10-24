//! The settings of a quiz, and its Wi-Fi slide.

use serde::{Deserialize, Serialize};

// The schema bounds the default time limit like a question's.
#[cfg(feature = "schema")]
use super::{MAX_TIME_LIMIT_SECS, MIN_TIME_LIMIT_SECS};

/// Longest Wi-Fi network name, in bytes (the limit of the standard).
pub const MAX_NETWORK_NAME_BYTES: usize = 32;

/// Longest Wi-Fi password, in characters (the limit of WPA).
pub const MAX_NETWORK_PASSWORD_CHARS: usize = 63;

/// Per-quiz options chosen by the admin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct QuizSettings {
    /// Time limit for questions that do not set their own.
    #[cfg_attr(feature = "schema", schemars(range(min = MIN_TIME_LIMIT_SECS, max = MAX_TIME_LIMIT_SECS)))]
    pub default_time_limit_secs: u32,
    pub reveal: RevealMode,
    pub multi_scoring: MultiScoring,
    pub points: PointsMode,
    /// Each team may double one question's points.
    pub joker: bool,
    pub advance: AdvanceMode,
    pub phone_display: PhoneDisplay,
    /// A first slide telling the room how to get on the Wi-Fi.
    pub network: NetworkSlide,
}

impl Default for QuizSettings {
    fn default() -> Self {
        Self {
            default_time_limit_secs: 20,
            reveal: RevealMode::default(),
            multi_scoring: MultiScoring::default(),
            points: PointsMode::default(),
            joker: false,
            advance: AdvanceMode::default(),
            phone_display: PhoneDisplay::default(),
            network: NetworkSlide::default(),
        }
    }
}

/// The Wi-Fi network teams join before the game, shown on a first slide with a QR code that
/// connects phones to it. The name and password are kept while the slide is off.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct NetworkSlide {
    /// Whether the game opens on the slide, before the join code.
    pub enabled: bool,
    /// The network name (SSID).
    pub name: String,
    /// Empty for an open network.
    pub password: String,
}

impl NetworkSlide {
    /// What keeps the slide from being shown, if anything.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        if !self.enabled {
            None
        } else if self.name.trim().is_empty() {
            Some("The Wi-Fi slide needs the network name.".to_owned())
        } else if self.name.len() > MAX_NETWORK_NAME_BYTES {
            Some(format!(
                "A Wi-Fi network name has at most {MAX_NETWORK_NAME_BYTES} characters."
            ))
        } else if self.password.chars().count() > MAX_NETWORK_PASSWORD_CHARS {
            Some(format!(
                "A Wi-Fi password has at most {MAX_NETWORK_PASSWORD_CHARS} characters."
            ))
        } else {
            None
        }
    }

    /// The text of a QR code that joins the network, as phone cameras read it.
    #[must_use]
    pub fn qr_text(&self) -> String {
        fn escape(text: &str) -> String {
            text.chars()
                .flat_map(|c| {
                    let escaped = matches!(c, '\\' | ';' | ',' | ':' | '"');
                    escaped.then_some('\\').into_iter().chain([c])
                })
                .collect()
        }
        let name = escape(&self.name);
        if self.password.is_empty() {
            format!("WIFI:T:nopass;S:{name};;")
        } else {
            format!("WIFI:T:WPA;S:{name};P:{};;", escape(&self.password))
        }
    }
}

/// When correct answers and scores are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum RevealMode {
    /// After every question, with the current standings.
    #[default]
    AfterEachQuestion,
    /// Only once all questions have been played, keeping the suspense.
    AtEnd,
}

/// How questions with several correct answers are scored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum MultiScoring {
    /// Points only for selecting exactly the correct set.
    #[default]
    AllOrNothing,
    /// Credit proportional to correct picks minus wrong picks.
    Partial,
}

/// What a correct answer is worth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PointsMode {
    /// Faster answers earn more: from 1000 points when instant down to 500 at the buzzer.
    #[default]
    Speed,
    /// Every correct answer earns 1000 points, however long it took.
    Flat,
}

/// Who moves the quiz forward between questions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum AdvanceMode {
    /// The host clicks to continue.
    #[default]
    Manual,
    /// The quiz continues by itself after a short pause.
    Auto,
}

/// What team leaders see on their phones while answering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PhoneDisplay {
    /// Only coloured shapes; the question is read on the big screen.
    #[default]
    ButtonsOnly,
    /// The question and answer texts as well.
    FullText,
}
