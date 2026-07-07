# AWS Segment Formatting

Nova supports a small AWS-specific format string so users can choose whether to
show the profile, region, or both without changing the segment layout.

## Current Behavior

`segments.aws.force_display` defaults to `true`. With the default behavior, Nova
renders AWS when either a profile or a region is available. Users only need to
set `force_display = false` when they want Starship-compatible strict credential
gating.

When `format` is omitted, Nova keeps the built-in display:

```text
 profile
 profile (region)
 (region)
```

## Format Strings

`segments.aws.format` is optional. When set, Nova uses it instead of the built-in
profile/region display.

Supported variables:

- `$symbol`: the configured AWS icon plus its trailing separator, or an empty
  string when `icon = ""`.
- `$profile`: the resolved AWS profile.
- `$region`: the resolved AWS region.
- `$duration`: reserved for temporary credential lifetime; currently renders as
  an empty string.

Optional groups use square brackets. A group renders only when at least one known
variable inside the group has a non-empty value.

Examples:

```toml
[segments.aws]
# Default-style custom format.
format = "$symbol$profile[ ($region)]"

# Hide region.
format = "$symbol$profile"

# Show only region.
format = "$symbol$region"
```

Unknown variables are left as literal text so typos are visible during manual
checks.

`style` applies to the entire rendered AWS segment. Nova does not yet support
per-variable AWS styles.

## Planned Duration Support

Starship's AWS module exposes a `$duration` variable for temporary credential
expiration. Nova should add this as a follow-up without changing the format
surface described above.

Duration sources should match Starship's practical behavior:

- `AWS_CREDENTIAL_EXPIRATION`
- `AWS_SESSION_EXPIRATION`
- `AWSUME_EXPIRATION`
- `expiration` or `x_security_token_expires` in the resolved credentials profile
- `expiresAt` in the AWS SSO cache for profiles that use `sso_session` or
  `sso_start_url`

If credentials are expired, Nova should render a short expiration marker instead
of a negative duration. If no expiration source is available, `$duration` should
remain empty and optional groups containing only `$duration` should disappear.

Duration lookup must not block the initial prompt on external commands. Reading
the same local AWS config, credentials, and SSO cache files already used for AWS
profile resolution is acceptable, but any slower or remote credential provider
execution belongs in an async collector.
