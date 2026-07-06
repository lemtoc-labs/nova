//! zsh-to-worker frame protocol.

use std::path::PathBuf;

use thiserror::Error;

use crate::render::LoweredPrompt;
use crate::state::{Keymap, PromptState};

pub const VERSION: &str = "1";
const FIELD_SEPARATOR: char = '\0';
const RECORD_SEPARATOR: char = '\x1e';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderRequest {
    pub generation: u64,
    pub state: PromptState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientRecord {
    Render(RenderRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerRecord {
    Handshake {
        session_token: String,
    },
    Prompt {
        generation: u64,
        status: RenderStatus,
        output: LoweredPrompt,
    },
    Update {
        generation: u64,
        status: RenderStatus,
        output: LoweredPrompt,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderStatus {
    Final,
    Partial,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("empty protocol record")]
    EmptyRecord,
    #[error("unknown protocol record `{0}`")]
    UnknownRecord(String),
    #[error("wrong field count for `{record}`: expected {expected}, got {actual}")]
    WrongFieldCount {
        record: String,
        expected: usize,
        actual: usize,
    },
    #[error("invalid unsigned integer in `{field}`: `{value}`")]
    InvalidUnsigned { field: String, value: String },
    #[error("invalid signed integer in `{field}`: `{value}`")]
    InvalidSigned { field: String, value: String },
    #[error("invalid columns value `{0}`")]
    InvalidColumns(String),
    #[error("invalid render status `{0}`")]
    InvalidStatus(String),
}

#[derive(Default)]
pub struct FrameDecoder {
    buffer: String,
}

impl FrameDecoder {
    pub fn push(&mut self, chunk: &str) -> Vec<String> {
        self.buffer.push_str(chunk);

        let mut frames = Vec::new();
        while let Some(position) = self.buffer.find(RECORD_SEPARATOR) {
            let record = self.buffer[..position].to_string();
            self.buffer.drain(..=position);
            frames.push(record);
        }

        frames
    }
}

pub fn encode_client_record(record: &ClientRecord) -> String {
    match record {
        ClientRecord::Render(request) => encode_fields(&[
            "R".to_string(),
            request.generation.to_string(),
            request.state.cwd.to_string_lossy().into_owned(),
            request.state.exit_status.to_string(),
            request
                .state
                .duration_ms
                .map(|duration_ms| duration_ms.to_string())
                .unwrap_or_default(),
            request.state.columns.to_string(),
            keymap_name(request.state.keymap).to_string(),
        ]),
    }
}

pub fn decode_client_record(record: &str) -> Result<ClientRecord, ProtocolError> {
    let fields = split_fields(record);
    let Some(record_type) = fields.first() else {
        return Err(ProtocolError::EmptyRecord);
    };

    match record_type.as_str() {
        "R" => {
            expect_field_count(record_type, fields.len(), 7)?;
            let generation = parse_u64("gen", &fields[1])?;
            let exit_status = parse_i32("exit_status", &fields[3])?;
            let duration_ms = if fields[4].is_empty() {
                None
            } else {
                Some(parse_u64("duration_ms", &fields[4])?)
            };
            let columns = parse_columns(&fields[5])?;
            let keymap = Keymap::parse(&fields[6]);

            Ok(ClientRecord::Render(RenderRequest {
                generation,
                state: PromptState {
                    cwd: PathBuf::from(&fields[2]),
                    exit_status,
                    duration_ms,
                    columns,
                    keymap,
                },
            }))
        }
        record_type => Err(ProtocolError::UnknownRecord(record_type.to_string())),
    }
}

pub fn encode_worker_record(record: &WorkerRecord) -> String {
    match record {
        WorkerRecord::Handshake { session_token } => encode_fields(&[
            "H".to_string(),
            VERSION.to_string(),
            clean_field(session_token),
        ]),
        WorkerRecord::Prompt {
            generation,
            status,
            output,
        } => encode_render_record("P", *generation, *status, output),
        WorkerRecord::Update {
            generation,
            status,
            output,
        } => encode_render_record("U", *generation, *status, output),
    }
}

pub fn decode_worker_record(record: &str) -> Result<WorkerRecord, ProtocolError> {
    let fields = split_fields(record);
    let Some(record_type) = fields.first() else {
        return Err(ProtocolError::EmptyRecord);
    };

    match record_type.as_str() {
        "H" => {
            expect_field_count(record_type, fields.len(), 3)?;
            Ok(WorkerRecord::Handshake {
                session_token: fields[2].clone(),
            })
        }
        "P" | "U" => {
            expect_field_count(record_type, fields.len(), 5)?;
            let generation = parse_u64("gen", &fields[1])?;
            let status = parse_status(&fields[2])?;
            let output = LoweredPrompt {
                prompt: fields[3].clone(),
                rprompt: fields[4].clone(),
            };

            if record_type == "P" {
                Ok(WorkerRecord::Prompt {
                    generation,
                    status,
                    output,
                })
            } else {
                Ok(WorkerRecord::Update {
                    generation,
                    status,
                    output,
                })
            }
        }
        record_type => Err(ProtocolError::UnknownRecord(record_type.to_string())),
    }
}

fn encode_render_record(
    record_type: &str,
    generation: u64,
    status: RenderStatus,
    output: &LoweredPrompt,
) -> String {
    encode_fields(&[
        record_type.to_string(),
        generation.to_string(),
        status_name(status).to_string(),
        clean_field(&output.prompt),
        clean_field(&output.rprompt),
    ])
}

fn encode_fields(fields: &[String]) -> String {
    let mut record = fields
        .iter()
        .map(|field| clean_field(field))
        .collect::<Vec<_>>()
        .join(&FIELD_SEPARATOR.to_string());
    record.push(RECORD_SEPARATOR);
    record
}

fn split_fields(record: &str) -> Vec<String> {
    record
        .split(FIELD_SEPARATOR)
        .map(ToString::to_string)
        .collect()
}

fn clean_field(input: &str) -> String {
    input
        .chars()
        .filter(|character| *character != FIELD_SEPARATOR && *character != RECORD_SEPARATOR)
        .collect()
}

fn expect_field_count(record: &str, actual: usize, expected: usize) -> Result<(), ProtocolError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProtocolError::WrongFieldCount {
            record: record.to_string(),
            expected,
            actual,
        })
    }
}

fn parse_u64(field: &str, value: &str) -> Result<u64, ProtocolError> {
    value
        .parse()
        .map_err(|_error| ProtocolError::InvalidUnsigned {
            field: field.to_string(),
            value: value.to_string(),
        })
}

fn parse_i32(field: &str, value: &str) -> Result<i32, ProtocolError> {
    value
        .parse()
        .map_err(|_error| ProtocolError::InvalidSigned {
            field: field.to_string(),
            value: value.to_string(),
        })
}

fn parse_columns(value: &str) -> Result<u16, ProtocolError> {
    let columns = value
        .parse::<u16>()
        .map_err(|_error| ProtocolError::InvalidColumns(value.to_string()))?;
    if columns == 0 {
        Err(ProtocolError::InvalidColumns(value.to_string()))
    } else {
        Ok(columns)
    }
}

fn parse_status(value: &str) -> Result<RenderStatus, ProtocolError> {
    match value {
        "final" => Ok(RenderStatus::Final),
        "partial" => Ok(RenderStatus::Partial),
        _ => Err(ProtocolError::InvalidStatus(value.to_string())),
    }
}

fn keymap_name(keymap: Keymap) -> &'static str {
    match keymap {
        Keymap::Main => "main",
        Keymap::ViCommand => "vicmd",
    }
}

fn status_name(status: RenderStatus) -> &'static str {
    match status {
        RenderStatus::Final => "final",
        RenderStatus::Partial => "partial",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_render_requests() {
        let record = ClientRecord::Render(RenderRequest {
            generation: 42,
            state: PromptState {
                cwd: PathBuf::from("/tmp/nova"),
                exit_status: 1,
                duration_ms: Some(123),
                columns: 80,
                keymap: Keymap::ViCommand,
            },
        });

        let encoded = encode_client_record(&record);
        let encoded = encoded.trim_end_matches(RECORD_SEPARATOR);

        assert_eq!(decode_client_record(encoded), Ok(record));
    }

    #[test]
    fn round_trips_worker_prompt_records() {
        let record = WorkerRecord::Prompt {
            generation: 7,
            status: RenderStatus::Final,
            output: LoweredPrompt {
                prompt: "left\nright".to_string(),
                rprompt: "duration".to_string(),
            },
        };

        let encoded = encode_worker_record(&record);
        let encoded = encoded.trim_end_matches(RECORD_SEPARATOR);

        assert_eq!(decode_worker_record(encoded), Ok(record));
    }

    #[test]
    fn decodes_torn_frames() {
        let first = ClientRecord::Render(RenderRequest {
            generation: 1,
            state: PromptState {
                cwd: PathBuf::from("/one"),
                exit_status: 0,
                duration_ms: None,
                columns: 80,
                keymap: Keymap::Main,
            },
        });
        let second = ClientRecord::Render(RenderRequest {
            generation: 2,
            state: PromptState {
                cwd: PathBuf::from("/two"),
                exit_status: 1,
                duration_ms: Some(5),
                columns: 40,
                keymap: Keymap::Main,
            },
        });
        let combined = format!(
            "{}{}",
            encode_client_record(&first),
            encode_client_record(&second)
        );

        for split_at in 0..combined.len() {
            let mut decoder = FrameDecoder::default();
            let mut frames = decoder.push(&combined[..split_at]);
            frames.extend(decoder.push(&combined[split_at..]));

            assert_eq!(frames.len(), 2);
            assert_eq!(decode_client_record(&frames[0]), Ok(first.clone()));
            assert_eq!(decode_client_record(&frames[1]), Ok(second.clone()));
        }
    }

    #[test]
    fn strips_protocol_separators_from_output_fields() {
        let encoded = encode_worker_record(&WorkerRecord::Handshake {
            session_token: "a\0b\x1ec".to_string(),
        });

        assert_eq!(encoded, format!("H\0{}\0abc\x1e", VERSION));
    }
}
