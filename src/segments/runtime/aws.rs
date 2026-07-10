use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::config::SegmentConfig;
use crate::segments::{SegmentContent, Style, SyncSegment, label_with_icon};
use crate::state::{PromptEnv, PromptState};

const AWS_SEGMENT_ID: &str = "aws";
const AWS_ICON: &str = "";

pub struct AwsSegment;

impl SyncSegment for AwsSegment {
    fn id(&self) -> &'static str {
        AWS_SEGMENT_ID
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        render_aws(&state.env, config)
    }
}

pub fn render_aws(env: &PromptEnv, config: &SegmentConfig) -> Option<SegmentContent> {
    let force_display = config.force_display.unwrap_or(true);
    let context = resolve_aws_context(env, force_display)?;
    let text = match config.format.as_deref() {
        Some(format) => render_aws_format(format, &context, config),
        None => label_with_icon(&context.label(), config, AWS_ICON),
    };
    if text.is_empty() {
        return None;
    }

    Some(SegmentContent::new(AWS_SEGMENT_ID, text, aws_style(config)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AwsContext {
    profile: Option<String>,
    region: Option<String>,
}

impl AwsContext {
    fn label(&self) -> String {
        match (self.profile.as_deref(), self.region.as_deref()) {
            (Some(profile), Some(region)) => format!("{profile} ({region})"),
            (Some(profile), None) => profile.to_string(),
            (None, Some(region)) => format!("({region})"),
            (None, None) => String::new(),
        }
    }
}

struct AwsFormatVariables<'a> {
    symbol: String,
    profile: Option<&'a str>,
    region: Option<&'a str>,
}

fn render_aws_format(format: &str, context: &AwsContext, config: &SegmentConfig) -> String {
    let variables = AwsFormatVariables {
        symbol: aws_symbol(config),
        profile: context.profile.as_deref(),
        region: context.region.as_deref(),
    };
    render_aws_format_template(format, &variables)
}

fn render_aws_format_template(format: &str, variables: &AwsFormatVariables<'_>) -> String {
    let chars = format.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut plain = String::new();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '['
            && let Some(end) = closing_optional_group(&chars, index + 1)
        {
            output.push_str(&render_aws_format_part(&plain, variables).text);
            plain.clear();
            let inner = chars[index + 1..end].iter().collect::<String>();
            let rendered = render_aws_format_part(&inner, variables);
            if rendered.has_value {
                output.push_str(&rendered.text);
            }
            index = end + 1;
            continue;
        }

        plain.push(chars[index]);
        index += 1;
    }

    output.push_str(&render_aws_format_part(&plain, variables).text);
    output
}

#[derive(Debug, PartialEq, Eq)]
struct RenderedAwsFormatPart {
    text: String,
    has_value: bool,
}

fn render_aws_format_part(
    input: &str,
    variables: &AwsFormatVariables<'_>,
) -> RenderedAwsFormatPart {
    let chars = input.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut has_value = false;
    let mut index = 0;

    while index < chars.len() {
        if chars[index] != '$' {
            output.push(chars[index]);
            index += 1;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while end < chars.len() && is_format_variable_char(chars[end]) {
            end += 1;
        }

        if start == end {
            output.push('$');
            index += 1;
            continue;
        }

        let name = chars[start..end].iter().collect::<String>();
        match aws_format_value(&name, variables) {
            Some(value) => {
                has_value |= !value.is_empty();
                output.push_str(&value);
            }
            None => {
                output.push('$');
                output.push_str(&name);
            }
        }
        index = end;
    }

    RenderedAwsFormatPart {
        text: output,
        has_value,
    }
}

fn closing_optional_group(chars: &[char], start: usize) -> Option<usize> {
    chars
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, character)| (*character == ']').then_some(index))
}

fn is_format_variable_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn aws_format_value(name: &str, variables: &AwsFormatVariables<'_>) -> Option<String> {
    match name {
        "symbol" => Some(variables.symbol.clone()),
        "profile" => Some(variables.profile.unwrap_or_default().to_string()),
        "region" => Some(variables.region.unwrap_or_default().to_string()),
        "duration" => Some(String::new()),
        _ => None,
    }
}

fn aws_symbol(config: &SegmentConfig) -> String {
    match config.icon.as_deref() {
        Some("") => String::new(),
        Some(icon) => format!("{icon} "),
        None => format!("{AWS_ICON} "),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct IniFile {
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl IniFile {
    fn section(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        self.sections.get(name)
    }
}

fn resolve_aws_context(env: &PromptEnv, force_display: bool) -> Option<AwsContext> {
    let config_file = aws_config_file_path(env).and_then(read_ini_file);
    let credentials_file = aws_credentials_file_path(env).and_then(read_ini_file);
    let profile = aws_profile(env);
    let region = aws_region(env, profile.as_ref(), config_file.as_ref());

    if profile.is_none() && region.is_none() {
        return None;
    }

    if !force_display
        && !has_credential_process_or_sso(
            config_file.as_ref(),
            credentials_file.as_ref(),
            profile.as_ref(),
        )
        && !has_source_profile(
            config_file.as_ref(),
            credentials_file.as_ref(),
            profile.as_ref(),
        )
        && !has_defined_credentials(env, credentials_file.as_ref(), profile.as_ref())
    {
        return None;
    }

    Some(AwsContext { profile, region })
}

fn aws_profile(env: &PromptEnv) -> Option<String> {
    [
        env.aws.awsu_profile.as_ref(),
        env.aws.aws_vault.as_ref(),
        env.aws.awsume_profile.as_ref(),
        env.aws.aws_profile.as_ref(),
        env.aws.aws_sso_profile.as_ref(),
    ]
    .into_iter()
    .flatten()
    .next()
    .cloned()
}

fn aws_region(
    env: &PromptEnv,
    profile: Option<&String>,
    config_file: Option<&IniFile>,
) -> Option<String> {
    env.aws
        .aws_region
        .clone()
        .or_else(|| env.aws.aws_default_region.clone())
        .or_else(|| {
            aws_config_section(config_file, profile)?
                .get("region")
                .cloned()
        })
}

fn has_credential_process_or_sso(
    config_file: Option<&IniFile>,
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    let config_has = aws_config_section(config_file, profile).is_some_and(|section| {
        section.contains_key("credential_process")
            || section.contains_key("sso_session")
            || section.contains_key("sso_start_url")
    });
    if config_has {
        return true;
    }

    aws_credentials_section(credentials_file, profile).is_some_and(|section| {
        section.contains_key("credential_process") || section.contains_key("sso_start_url")
    })
}

fn has_source_profile(
    config_file: Option<&IniFile>,
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    let Some(source_profile) =
        aws_config_section(config_file, profile).and_then(|section| section.get("source_profile"))
    else {
        return false;
    };

    has_credential_process_or_sso(config_file, credentials_file, Some(source_profile))
        || has_defined_credentials_for_profile(credentials_file, Some(source_profile))
}

fn has_defined_credentials(
    env: &PromptEnv,
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    env.aws.aws_access_key_id_present
        || env.aws.aws_secret_access_key_present
        || env.aws.aws_session_token_present
        || has_defined_credentials_for_profile(credentials_file, profile)
}

fn has_defined_credentials_for_profile(
    credentials_file: Option<&IniFile>,
    profile: Option<&String>,
) -> bool {
    aws_credentials_section(credentials_file, profile)
        .is_some_and(|section| section.contains_key("aws_access_key_id"))
}

fn aws_config_section<'a>(
    config_file: Option<&'a IniFile>,
    profile: Option<&String>,
) -> Option<&'a BTreeMap<String, String>> {
    let section_name = match profile {
        Some(profile) => format!("profile {profile}"),
        None => "default".to_string(),
    };
    config_file?.section(&section_name)
}

fn aws_credentials_section<'a>(
    credentials_file: Option<&'a IniFile>,
    profile: Option<&String>,
) -> Option<&'a BTreeMap<String, String>> {
    let section_name = profile.map_or("default", String::as_str);
    credentials_file?.section(section_name)
}

fn aws_config_file_path(env: &PromptEnv) -> Option<PathBuf> {
    env.aws
        .aws_config_file
        .clone()
        .or_else(|| env.home.as_ref().map(|home| home.join(".aws/config")))
}

fn aws_credentials_file_path(env: &PromptEnv) -> Option<PathBuf> {
    env.aws
        .aws_shared_credentials_file
        .clone()
        .or_else(|| env.aws.aws_credentials_file.clone())
        .or_else(|| env.home.as_ref().map(|home| home.join(".aws/credentials")))
}

fn read_ini_file(path: PathBuf) -> Option<IniFile> {
    fs::read_to_string(path)
        .ok()
        .map(|contents| parse_ini(&contents))
}

fn parse_ini(input: &str) -> IniFile {
    let mut file = IniFile::default();
    let mut current_section = None;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
            .map(str::trim)
            .filter(|section| !section.is_empty())
        {
            current_section = Some(section.to_string());
            file.sections.entry(section.to_string()).or_default();
            continue;
        }

        let Some(section) = current_section.as_ref() else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }

        file.sections
            .entry(section.clone())
            .or_default()
            .insert(key.to_string(), value.trim().to_string());
    }

    file
}

fn aws_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("yellow".to_string()),
            bg: None,
            bold: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AwsEnv;

    #[test]
    fn renders_aws_region_with_env_credentials() {
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_region: Some("ap-northeast-1".to_string()),
                    aws_access_key_id_present: true,
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.id, "aws");
        assert_eq!(segment.text, " (ap-northeast-1)");
        assert_eq!(segment.style.fg.as_deref(), Some("yellow"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_aws_without_credentials_by_default() {
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_region: Some("ap-northeast-1".to_string()),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (ap-northeast-1)");
    }

    #[test]
    fn renders_aws_with_configured_format_hiding_region() {
        let config = SegmentConfig {
            format: Some("$symbol$profile".to_string()),
            ..SegmentConfig::default()
        };
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_region: Some("ap-northeast-1".to_string()),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &config,
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts");
    }

    #[test]
    fn renders_aws_optional_format_groups_only_when_variables_are_present() {
        let config = SegmentConfig {
            format: Some("$symbol$profile[ ($region)]".to_string()),
            ..SegmentConfig::default()
        };
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &config,
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts");
    }

    #[test]
    fn omits_aws_when_configured_format_renders_empty() {
        let config = SegmentConfig {
            icon: Some(String::new()),
            format: Some("$symbol".to_string()),
            ..SegmentConfig::default()
        };

        assert_eq!(
            render_aws(
                &PromptEnv {
                    aws: AwsEnv {
                        aws_profile: Some("astronauts".to_string()),
                        ..AwsEnv::default()
                    },
                    ..PromptEnv::default()
                },
                &config,
            ),
            None
        );
    }

    #[test]
    fn omits_aws_without_credentials_when_force_display_is_false() {
        let config = SegmentConfig {
            force_display: Some(false),
            ..SegmentConfig::default()
        };

        assert_eq!(
            render_aws(
                &PromptEnv {
                    aws: AwsEnv {
                        aws_profile: Some("astronauts".to_string()),
                        aws_region: Some("ap-northeast-1".to_string()),
                        ..AwsEnv::default()
                    },
                    ..PromptEnv::default()
                },
                &config,
            ),
            None
        );
    }

    #[test]
    fn resolves_aws_profile_using_starship_env_precedence() {
        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    awsu_profile: Some("awsu-profile".to_string()),
                    aws_vault: Some("vault-profile".to_string()),
                    awsume_profile: Some("awsume-profile".to_string()),
                    aws_profile: Some("plain-profile".to_string()),
                    aws_sso_profile: Some("sso-profile".to_string()),
                    aws_access_key_id_present: true,
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " awsu-profile");
    }

    #[test]
    fn reads_aws_profile_region_and_credential_process_from_config() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(
            &config_path,
            r#"
            [default]
            region = us-east-1

            [profile astronauts]
            region = ap-northeast-1
            credential_process = /opt/bin/awscreds-retriever
            "#,
        )
        .expect("config should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_config_file: Some(config_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (ap-northeast-1)");
    }

    #[test]
    fn accepts_aws_sso_config() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(
            &config_path,
            r#"
            [profile astronauts]
            region = us-east-2
            sso_start_url = https://example.com/start
            "#,
        )
        .expect("config should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_config_file: Some(config_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (us-east-2)");
    }

    #[test]
    fn reads_aws_default_region_and_default_credentials_from_files() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(&config_path, "[default]\nregion = us-east-1\n")
            .expect("config should be written");
        let credentials_path = tempdir.path().join("credentials");
        fs::write(&credentials_path, "[default]\naws_access_key_id = dummy\n")
            .expect("credentials should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_config_file: Some(config_path),
                    aws_shared_credentials_file: Some(credentials_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " (us-east-1)");
    }

    #[test]
    fn accepts_aws_source_profile_credentials() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let config_path = tempdir.path().join("config");
        fs::write(
            &config_path,
            r#"
            [profile astronauts]
            region = us-west-2
            source_profile = base
            "#,
        )
        .expect("config should be written");
        let credentials_path = tempdir.path().join("credentials");
        fs::write(&credentials_path, "[base]\naws_access_key_id = dummy\n")
            .expect("credentials should be written");

        let segment = render_aws(
            &PromptEnv {
                aws: AwsEnv {
                    aws_profile: Some("astronauts".to_string()),
                    aws_config_file: Some(config_path),
                    aws_shared_credentials_file: Some(credentials_path),
                    ..AwsEnv::default()
                },
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("aws segment should render");

        assert_eq!(segment.text, " astronauts (us-west-2)");
    }
}
