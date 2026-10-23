//! Bounded protocol shared by the Windows background and Settings processes.

use crate::{
    config::AppConfig,
    domain::{ActionId, CustomAction},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::IpAddr,
    path::PathBuf,
};

pub const PROTOCOL_VERSION: u16 = 4;
// Covers the serialized worst case from the existing 128-action and per-argument limits.
pub const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_PENDING_REQUESTS: usize = 1;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    pub version: u16,
    pub id: u64,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(id: u64, payload: T) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id,
            payload,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    Snapshot,
    SetListener { enabled: bool },
    SaveActions { actions: Vec<CustomAction> },
    ApplyVoiceConfig,
    TestAction { action_id: ActionId },
    SetStartup { enabled: bool },
    Reconfigure { bind_address: IpAddr, port: u16 },
    RevealPairingToken,
    ExportCa,
    RotateCredentials,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Response {
    Snapshot { state: DaemonSnapshot },
    Applied { state: DaemonSnapshot },
    ActionCancelled { state: DaemonSnapshot },
    PairingToken { token: String },
    CaExported { path: PathBuf },
    Error { code: ManagementError },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementError {
    InvalidRequest,
    Busy,
    Cancelled,
    Failed,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonSnapshot {
    pub config: AppConfig,
    pub diagnostics: Vec<DiagnosticSnapshot>,
    pub startup_error: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticSnapshot {
    pub request_id: Option<String>,
    pub action_id: Option<String>,
    pub category: String,
    pub reason: Option<String>,
}

pub fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut prefix = [0; 4];
    match reader.read(&mut prefix[..1])? {
        0 => return Ok(None),
        1 => reader.read_exact(&mut prefix[1..])?,
        _ => unreachable!(),
    }
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid IPC frame size",
        ));
    }
    let mut frame = vec![0; length];
    reader.read_exact(&mut frame)?;
    Ok(Some(frame))
}

pub fn write_frame(writer: &mut impl Write, frame: &[u8]) -> io::Result<()> {
    if frame.is_empty() || frame.len() > MAX_FRAME_BYTES || frame.len() > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid IPC frame size",
        ));
    }
    writer.write_all(&(frame.len() as u32).to_be_bytes())?;
    writer.write_all(frame)?;
    writer.flush()
}

pub fn read_message<T: for<'de> Deserialize<'de>>(reader: &mut impl Read) -> io::Result<Option<T>> {
    read_frame(reader)?
        .map(|frame| {
            serde_json::from_slice(&frame)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid IPC message"))
        })
        .transpose()
}

pub fn write_message<T: Serialize>(writer: &mut impl Write, message: &T) -> io::Result<()> {
    let mut frame = LimitedFrame(Vec::new());
    serde_json::to_writer(&mut frame, message)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "IPC serialization failed"))?;
    write_frame(writer, &frame.0)
}

struct LimitedFrame(Vec<u8>);

impl Write for LimitedFrame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "IPC frame exceeds limit",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn request_round_trips_with_version_and_correlation_id() {
        let request = Envelope::new(42, Request::SetListener { enabled: true });
        let mut bytes = Vec::new();
        write_message(&mut bytes, &request).unwrap();
        let decoded: Envelope<Request> = read_message(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.id, 42);
        assert!(matches!(
            decoded.payload,
            Request::SetListener { enabled: true }
        ));
    }

    #[test]
    fn apply_voice_config_is_a_versioned_management_request() {
        let request = Envelope::new(7, Request::ApplyVoiceConfig);
        let mut bytes = Vec::new();
        write_message(&mut bytes, &request).unwrap();
        let decoded: Envelope<Request> = read_message(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert!(matches!(decoded.payload, Request::ApplyVoiceConfig));
    }

    #[test]
    fn diagnostic_snapshot_serializes_failure_reason() {
        let snapshot = DiagnosticSnapshot {
            request_id: Some("request".into()),
            action_id: Some("launch_game".into()),
            category: "action_failed".into(),
            reason: Some("executable not found (OS error 2)".into()),
        };
        let value = serde_json::to_value(snapshot).unwrap();
        assert_eq!(value["reason"], "executable not found (OS error 2)");
    }

    #[test]
    fn action_cancellation_response_preserves_the_updated_snapshot() {
        let response = Envelope::new(
            8,
            Response::ActionCancelled {
                state: DaemonSnapshot {
                    config: AppConfig::default(),
                    diagnostics: Vec::new(),
                    startup_error: false,
                },
            },
        );
        let mut bytes = Vec::new();
        write_message(&mut bytes, &response).unwrap();
        let decoded: Envelope<Response> = read_message(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert!(matches!(decoded.payload, Response::ActionCancelled { .. }));
    }

    #[test]
    fn frames_reject_empty_oversized_and_truncated_data() {
        for bytes in [
            vec![0, 0, 0, 0],
            (MAX_FRAME_BYTES as u32 + 1).to_be_bytes().to_vec(),
            vec![0, 0, 0, 2, b'{'],
        ] {
            assert!(read_frame(&mut Cursor::new(bytes)).is_err());
        }
        assert!(write_frame(&mut Vec::new(), &[]).is_err());
    }

    #[test]
    fn clean_eof_is_distinct_from_a_truncated_frame() {
        assert!(read_frame(&mut Cursor::new([])).unwrap().is_none());
        assert!(read_frame(&mut Cursor::new([0, 0])).is_err());
    }

    #[test]
    fn unknown_messages_and_fields_are_rejected() {
        for json in [
            br#"{"version":1,"id":1,"payload":{"type":"exec","path":"C:/bad.exe"}}"#.as_slice(),
            br#"{"version":1,"id":1,"extra":true,"payload":{"type":"snapshot"}}"#.as_slice(),
            br#"{"version":1,"id":1,"payload":{"type":"snapshot","extra":true}}"#.as_slice(),
        ] {
            assert!(
                serde_json::from_slice::<Envelope<Request>>(json).is_err(),
                "accepted: {}",
                String::from_utf8_lossy(json)
            );
        }
    }

    #[test]
    fn general_snapshot_does_not_contain_pairing_token_or_private_key_fields() {
        let json = serde_json::to_value(DaemonSnapshot {
            config: AppConfig::default(),
            diagnostics: Vec::new(),
            startup_error: false,
        })
        .unwrap();
        assert!(json.get("token").is_none());
        assert!(json.get("private_key").is_none());
    }
}
