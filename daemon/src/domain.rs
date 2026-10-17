use serde::{Deserialize, Serialize, de};
use std::{fmt, path::PathBuf};

pub const BODY_LIMIT: usize = 512;
pub const REQUEST_LIFETIME_MS: u64 = 5_000;
pub const MAX_FUTURE_SKEW_MS: u64 = 1_000;
pub const MAX_REMEMBERED_IDS: usize = 128;
pub const VOICE_CONFIG_SCHEMA: u16 = 1;
pub const VOICE_CONFIG_LIMIT: usize = 3072;
pub const MAX_VOICE_ACTIONS: usize = 12;
pub const MAX_VOICE_PHRASE_BYTES: usize = 63;

pub fn is_safe_batch_literal(value: &str) -> bool {
    !value.is_empty()
        && !value
            .chars()
            .any(|ch| matches!(ch, '"' | '%' | '!' | '\r' | '\n' | '\0'))
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || " ._-/\\:,+()[]=".contains(ch))
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RequestId(String);

impl RequestId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.len() != 32
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ValidationError::InvalidId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RequestId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RequestId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ActionId(String);

impl ActionId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        let valid = (1..=64).contains(&value.len())
            && value.as_bytes()[0].is_ascii_lowercase()
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if valid {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidActionId)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ActionId {
    type Error = ValidationError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}
impl From<ActionId> for String {
    fn from(value: ActionId) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaCommand {
    Play,
    Pause,
    Next,
    Previous,
    Mute,
    Unmute,
}

impl MediaCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Next => "next",
            Self::Previous => "previous",
            Self::Mute => "mute",
            Self::Unmute => "unmute",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaRequest {
    pub id: RequestId,
    pub cmd: MediaCommand,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomActionRequest {
    pub id: RequestId,
    pub action_id: ActionId,
    pub voice_revision: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceCommand {
    pub phrase: String,
    pub action_id: ActionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedVoiceConfig {
    pub schema: u16,
    pub revision: u64,
    pub actions_revision: u64,
    pub commands: Vec<VoiceCommand>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceConfigAckRequest {
    pub revision: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConfiguredAction {
    Media {
        command: MediaCommand,
    },
    Launch {
        program: PathBuf,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
    },
    Batch {
        script: PathBuf,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
    },
    PowerShell {
        interpreter: PathBuf,
        script: PathBuf,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomAction {
    pub id: ActionId,
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub phrase: Option<String>,
    pub action: ConfiguredAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    InvalidId,
    InvalidActionId,
    InvalidTime,
    InvalidVoiceConfig,
    TooLarge,
    InvalidJson,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidId => "invalid request id",
            Self::InvalidActionId => "invalid action id",
            Self::InvalidTime => "invalid request time",
            Self::InvalidVoiceConfig => "invalid voice configuration",
            Self::TooLarge => "request body too large",
            Self::InvalidJson => "invalid request body",
        })
    }
}
impl std::error::Error for ValidationError {}

pub fn validate_voice_commands(commands: &[VoiceCommand]) -> Result<(), ValidationError> {
    if commands.len() > MAX_VOICE_ACTIONS {
        return Err(ValidationError::InvalidVoiceConfig);
    }
    validate_voice_command_drafts(commands)
}

pub fn validate_voice_command_drafts(commands: &[VoiceCommand]) -> Result<(), ValidationError> {
    let reserved = [
        "jarvis", "wake", "play", "pause", "next", "back", "mute", "unmute", "update",
    ];
    let mut phrases = std::collections::HashSet::with_capacity(commands.len());
    let mut action_ids = std::collections::HashSet::with_capacity(commands.len());
    for command in commands {
        let phrase = command.phrase.as_str();
        if !(2..=MAX_VOICE_PHRASE_BYTES).contains(&phrase.len())
            || phrase.trim() != phrase
            || !phrase.is_ascii()
            || !phrase
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || byte == b' ')
            || phrase.as_bytes().windows(2).any(|pair| pair == b"  ")
        {
            return Err(ValidationError::InvalidVoiceConfig);
        }
        let normalized = phrase.to_ascii_lowercase();
        if reserved.contains(&normalized.as_str())
            || !phrases.insert(normalized)
            || !action_ids.insert(command.action_id.as_str())
        {
            return Err(ValidationError::InvalidVoiceConfig);
        }
    }
    Ok(())
}

pub fn parse_request<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, ValidationError> {
    if body.len() > BODY_LIMIT {
        return Err(ValidationError::TooLarge);
    }
    serde_json::from_slice(body).map_err(|_| ValidationError::InvalidJson)
}

pub fn validate_time(issued_ms: u64, expires_ms: u64, now_ms: u64) -> Result<(), ValidationError> {
    let expected_expiry = issued_ms
        .checked_add(REQUEST_LIFETIME_MS)
        .ok_or(ValidationError::InvalidTime)?;
    if expires_ms != expected_expiry
        || now_ms >= expires_ms
        || (issued_ms > now_ms && issued_ms - now_ms > MAX_FUTURE_SKEW_MS)
    {
        return Err(ValidationError::InvalidTime);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> &'static str {
        "0123456789abcdef0123456789abcdef"
    }

    #[test]
    fn request_id_requires_exact_lower_hex() {
        assert!(RequestId::parse(id()).is_ok());
        assert_eq!(
            RequestId::parse("0123456789ABCDEF0123456789ABCDEF"),
            Err(ValidationError::InvalidId)
        );
        assert_eq!(RequestId::parse("short"), Err(ValidationError::InvalidId));
    }

    #[test]
    fn parser_rejects_extra_fields_duplicate_fields_and_oversize() {
        let extra = format!(
            r#"{{"id":"{}","cmd":"pause","issued_at_ms":100,"expires_at_ms":5100,"x":1}}"#,
            id()
        );
        assert_eq!(
            parse_request::<MediaRequest>(extra.as_bytes()).unwrap_err(),
            ValidationError::InvalidJson
        );
        let duplicate = format!(
            r#"{{"id":"{}","id":"{}","cmd":"pause","issued_at_ms":100,"expires_at_ms":5100}}"#,
            id(),
            id()
        );
        assert_eq!(
            parse_request::<MediaRequest>(duplicate.as_bytes()).unwrap_err(),
            ValidationError::InvalidJson
        );
        assert_eq!(
            parse_request::<MediaRequest>(&vec![b' '; BODY_LIMIT + 1]).unwrap_err(),
            ValidationError::TooLarge
        );
    }

    #[test]
    fn time_validation_checks_expiry_boundary_skew_and_overflow() {
        assert!(validate_time(1_000, 6_000, 1_000).is_ok());
        assert_eq!(
            validate_time(1_000, 6_000, 6_000),
            Err(ValidationError::InvalidTime)
        );
        assert_eq!(
            validate_time(1_000, 6_001, 1_000),
            Err(ValidationError::InvalidTime)
        );
        assert_eq!(
            validate_time(2_001, 7_001, 1_000),
            Err(ValidationError::InvalidTime)
        );
        assert_eq!(
            validate_time(u64::MAX, 4, 1),
            Err(ValidationError::InvalidTime)
        );
    }

    #[test]
    fn batch_literals_are_quoted_safe_or_rejected() {
        assert!(is_safe_batch_literal(
            r#"C:\Program Files (x86)\Player\run.cmd"#
        ));
        assert!(is_safe_batch_literal("argument with spaces"));
        for unsafe_value in ["%TEMP%", "!VAR!", "a&b", "a|b", "a^b", "quote\\\"x", "a\nb"] {
            assert!(
                !is_safe_batch_literal(unsafe_value),
                "accepted unsafe batch value: {unsafe_value:?}"
            );
        }
    }

    #[test]
    fn action_ids_have_a_stable_safe_grammar() {
        assert!(ActionId::parse("launch_spotify").is_ok());
        assert!(ActionId::parse("1launch").is_err());
        assert!(ActionId::parse("Launch").is_err());
        assert!(ActionId::parse("run-script").is_err());
    }

    #[test]
    fn voice_phrases_are_bounded_unique_and_non_reserved() {
        let command = |phrase: &str, id: &str| VoiceCommand {
            phrase: phrase.into(),
            action_id: ActionId::parse(id).unwrap(),
        };
        assert!(validate_voice_commands(&[command("open spotify", "spotify")]).is_ok());
        assert!(validate_voice_commands(&[command("sync", "sync_action")]).is_ok());
        for invalid in [
            "update",
            "Jarvis",
            "play",
            "open  spotify",
            " open app",
            "open!",
            "x",
            &"a".repeat(64),
        ] {
            assert!(
                validate_voice_commands(&[command(invalid, "action")]).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(
            validate_voice_commands(&[
                command("open spotify", "one"),
                command("OPEN SPOTIFY", "two"),
            ])
            .is_err()
        );
        assert!(
            validate_voice_commands(&[
                command("open spotify", "same"),
                command("close spotify", "same"),
            ])
            .is_err()
        );
        let too_many: Vec<_> = (0..=MAX_VOICE_ACTIONS)
            .map(|index| {
                let letters = (b'a' + index as u8) as char;
                command(&format!("open app {letters}"), &format!("app{index}"))
            })
            .collect();
        assert!(validate_voice_commands(&too_many).is_err());

        let maximum: Vec<_> = (0..MAX_VOICE_ACTIONS)
            .map(|index| {
                let suffix = (b'a' + index as u8) as char;
                command(
                    &format!("open {}{suffix}", "a".repeat(57)),
                    &format!("app{index}"),
                )
            })
            .collect();
        assert!(validate_voice_commands(&maximum).is_ok());
        let published = PublishedVoiceConfig {
            schema: VOICE_CONFIG_SCHEMA,
            revision: 1,
            actions_revision: 1,
            commands: maximum,
        };
        assert!(serde_json::to_vec(&published).unwrap().len() <= VOICE_CONFIG_LIMIT);
    }
}
