//! zsh-to-worker frame protocol.

use std::path::PathBuf;

use thiserror::Error;

use crate::render::LoweredPrompt;
use crate::state::{AwsEnv, Keymap, PromptEnv, PromptState};

pub const VERSION: &str = "6";
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
            request.state.env.user.clone().unwrap_or_default(),
            request.state.env.host.clone().unwrap_or_default(),
            request.state.time.clone().unwrap_or_default(),
            request
                .state
                .env
                .virtual_env
                .as_ref()
                .map(|virtual_env| virtual_env.to_string_lossy().into_owned())
                .unwrap_or_default(),
            request.state.env.in_nix_shell.clone().unwrap_or_default(),
            request.state.env.nix_shell_name.clone().unwrap_or_default(),
            request
                .state
                .env
                .nix_shell_level
                .clone()
                .unwrap_or_default(),
            request
                .state
                .env
                .home
                .as_ref()
                .map(|home| home.to_string_lossy().into_owned())
                .unwrap_or_default(),
            request
                .state
                .env
                .aws
                .awsu_profile
                .clone()
                .unwrap_or_default(),
            request.state.env.aws.aws_vault.clone().unwrap_or_default(),
            request
                .state
                .env
                .aws
                .awsume_profile
                .clone()
                .unwrap_or_default(),
            request
                .state
                .env
                .aws
                .aws_profile
                .clone()
                .unwrap_or_default(),
            request
                .state
                .env
                .aws
                .aws_sso_profile
                .clone()
                .unwrap_or_default(),
            request.state.env.aws.aws_region.clone().unwrap_or_default(),
            request
                .state
                .env
                .aws
                .aws_default_region
                .clone()
                .unwrap_or_default(),
            request
                .state
                .env
                .aws
                .aws_config_file
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            request
                .state
                .env
                .aws
                .aws_shared_credentials_file
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            request
                .state
                .env
                .aws
                .aws_credentials_file
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            bool_field(request.state.env.aws.aws_access_key_id_present),
            bool_field(request.state.env.aws.aws_secret_access_key_present),
            bool_field(request.state.env.aws.aws_session_token_present),
            request.state.env.path.clone().unwrap_or_default(),
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
            expect_render_field_count(record_type, fields.len())?;
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
                    time: non_empty_string(&fields[9]),
                    columns,
                    keymap,
                    env: PromptEnv {
                        user: non_empty_string(&fields[7]),
                        host: non_empty_string(&fields[8]),
                        path: fields.get(28).and_then(|value| non_empty_string(value)),
                        virtual_env: non_empty_path(&fields[10]),
                        in_nix_shell: non_empty_string(&fields[11]),
                        nix_shell_name: non_empty_string(&fields[12]),
                        nix_shell_level: non_empty_string(&fields[13]),
                        home: non_empty_path(&fields[14]),
                        aws: AwsEnv {
                            awsu_profile: non_empty_string(&fields[15]),
                            aws_vault: non_empty_string(&fields[16]),
                            awsume_profile: non_empty_string(&fields[17]),
                            aws_profile: non_empty_string(&fields[18]),
                            aws_sso_profile: non_empty_string(&fields[19]),
                            aws_region: non_empty_string(&fields[20]),
                            aws_default_region: non_empty_string(&fields[21]),
                            aws_config_file: non_empty_path(&fields[22]),
                            aws_shared_credentials_file: non_empty_path(&fields[23]),
                            aws_credentials_file: non_empty_path(&fields[24]),
                            aws_access_key_id_present: parse_bool_field(&fields[25]),
                            aws_secret_access_key_present: parse_bool_field(&fields[26]),
                            aws_session_token_present: parse_bool_field(&fields[27]),
                        },
                    },
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

fn expect_render_field_count(record: &str, actual: usize) -> Result<(), ProtocolError> {
    if actual == 28 || actual == 29 {
        Ok(())
    } else {
        Err(ProtocolError::WrongFieldCount {
            record: record.to_string(),
            expected: 29,
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

fn bool_field(value: bool) -> String {
    if value {
        "1".to_string()
    } else {
        String::new()
    }
}

fn parse_bool_field(value: &str) -> bool {
    !value.is_empty()
}

fn non_empty_path(value: &str) -> Option<PathBuf> {
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
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
                time: Some("11:16:42".to_string()),
                columns: 80,
                keymap: Keymap::ViCommand,
                env: PromptEnv {
                    user: Some("nova".to_string()),
                    host: Some("M4Pro".to_string()),
                    path: Some("/opt/nova/bin:/usr/bin:/bin".to_string()),
                    virtual_env: Some(PathBuf::from("/tmp/nova-venv")),
                    in_nix_shell: Some("pure".to_string()),
                    nix_shell_name: Some("nova".to_string()),
                    nix_shell_level: Some("1".to_string()),
                    home: Some(PathBuf::from("/home/nova")),
                    aws: AwsEnv {
                        awsu_profile: Some("awsu".to_string()),
                        aws_vault: Some("vault".to_string()),
                        awsume_profile: Some("awsume".to_string()),
                        aws_profile: Some("profile".to_string()),
                        aws_sso_profile: Some("sso".to_string()),
                        aws_region: Some("ap-northeast-1".to_string()),
                        aws_default_region: Some("us-east-1".to_string()),
                        aws_config_file: Some(PathBuf::from("/tmp/aws-config")),
                        aws_shared_credentials_file: Some(PathBuf::from("/tmp/aws-credentials")),
                        aws_credentials_file: Some(PathBuf::from("/tmp/aws-credentials-legacy")),
                        aws_access_key_id_present: true,
                        aws_secret_access_key_present: true,
                        aws_session_token_present: true,
                    },
                },
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
    fn decodes_render_requests_without_path_field() {
        let record = ClientRecord::Render(RenderRequest {
            generation: 7,
            state: PromptState {
                cwd: PathBuf::from("/tmp/nova"),
                exit_status: 0,
                duration_ms: None,
                time: None,
                columns: 80,
                keymap: Keymap::Main,
                env: PromptEnv::default(),
            },
        });
        let encoded = encode_client_record(&record);
        let frame = encoded.trim_end_matches(RECORD_SEPARATOR);
        let legacy_frame = frame
            .strip_suffix(FIELD_SEPARATOR)
            .expect("empty PATH field should be last");

        assert_eq!(legacy_frame.split(FIELD_SEPARATOR).count(), 28);
        assert_eq!(decode_client_record(legacy_frame), Ok(record));
    }

    #[test]
    fn strips_protocol_separators_from_client_path_field() {
        let record = ClientRecord::Render(RenderRequest {
            generation: 7,
            state: PromptState {
                cwd: PathBuf::from("/tmp/nova"),
                exit_status: 0,
                duration_ms: None,
                time: None,
                columns: 80,
                keymap: Keymap::Main,
                env: PromptEnv {
                    path: Some("a\0b\x1ec".to_string()),
                    ..PromptEnv::default()
                },
            },
        });
        let encoded = encode_client_record(&record);
        let frame = encoded.trim_end_matches(RECORD_SEPARATOR);
        let decoded = match decode_client_record(frame).expect("record should decode") {
            ClientRecord::Render(request) => request,
        };

        assert_eq!(decoded.state.env.path.as_deref(), Some("abc"));
    }

    #[test]
    fn decodes_torn_frames() {
        let first = ClientRecord::Render(RenderRequest {
            generation: 1,
            state: PromptState {
                cwd: PathBuf::from("/one"),
                exit_status: 0,
                duration_ms: None,
                time: None,
                columns: 80,
                keymap: Keymap::Main,
                env: PromptEnv::default(),
            },
        });
        let second = ClientRecord::Render(RenderRequest {
            generation: 2,
            state: PromptState {
                cwd: PathBuf::from("/two"),
                exit_status: 1,
                duration_ms: Some(5),
                time: Some("11:16:42".to_string()),
                columns: 40,
                keymap: Keymap::Main,
                env: PromptEnv::default(),
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
