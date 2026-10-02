//! Shared, fallible date rendering for the daemon and offline previews.
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, Utc};

use crate::config::VariableParams;

pub fn validate(params: &VariableParams) -> Result<()> {
    let format = format(params);
    if chrono::format::StrftimeItems::new(format)
        .any(|item| matches!(item, chrono::format::Item::Error))
    {
        bail!("invalid date format '{format}'");
    }
    // Exercise checked arithmetic and formatting as well as syntax. Rendering
    // repeats these checks because the clock can change after config loading.
    render(params, Utc::now())?;
    Ok(())
}

fn format(params: &VariableParams) -> &str {
    params
        .format
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("%Y-%m-%d")
}

pub fn render(params: &VariableParams, now: DateTime<Utc>) -> Result<String> {
    let offset = params.offset.unwrap_or(0);
    let duration = chrono::TimeDelta::try_seconds(offset)
        .context("date offset is outside the supported range")?;
    let instant = now
        .checked_add_signed(duration)
        .context("date offset puts the result outside the supported date range")?;
    let mut output = String::new();
    match params.tz.as_deref() {
        Some(name) => {
            let zone: chrono_tz::Tz = name.parse()
                .with_context(|| format!("unknown date timezone '{name}'; use an IANA name such as UTC or America/Chicago"))?;
            instant
                .with_timezone(&zone)
                .format(format(params))
                .write_to(&mut output)
                .context("could not format date")?;
        }
        None => {
            instant
                .with_timezone(&Local)
                .format(format(params))
                .write_to(&mut output)
                .context("could not format date")?;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(tz: &str) -> VariableParams {
        VariableParams {
            format: Some("%Y-%m-%d %H:%M %:z".into()),
            tz: Some(tz.into()),
            ..Default::default()
        }
    }

    fn instant(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn zones_follow_dst_and_calendar_boundaries() {
        let before = instant("2026-03-08T07:30:00Z");
        let after = instant("2026-03-08T08:30:00Z");
        assert_eq!(
            render(&params("America/Chicago"), before).unwrap(),
            "2026-03-08 01:30 -06:00"
        );
        assert_eq!(
            render(&params("America/Chicago"), after).unwrap(),
            "2026-03-08 03:30 -05:00"
        );
        assert_eq!(
            render(&params("UTC"), before).unwrap(),
            "2026-03-08 07:30 +00:00"
        );
        assert_eq!(
            render(&params("Asia/Tokyo"), instant("2026-12-31T20:00:00Z")).unwrap(),
            "2027-01-01 05:00 +09:00"
        );
        assert_eq!(
            render(&params("America/Chicago"), instant("2026-11-01T06:30:00Z")).unwrap(),
            "2026-11-01 01:30 -05:00"
        );
        assert_eq!(
            render(&params("America/Chicago"), instant("2026-11-01T07:30:00Z")).unwrap(),
            "2026-11-01 01:30 -06:00"
        );
    }

    #[test]
    fn offsets_are_elapsed_seconds_and_can_be_negative() {
        let mut p = params("America/Chicago");
        p.offset = Some(86400);
        assert_eq!(
            render(&p, instant("2026-03-07T18:00:00Z")).unwrap(),
            "2026-03-08 13:00 -05:00"
        );
        p.offset = Some(-3600);
        assert_eq!(
            render(&p, instant("2026-03-08T08:30:00Z")).unwrap(),
            "2026-03-08 01:30 -06:00"
        );
    }

    #[test]
    fn invalid_dates_return_errors_instead_of_panicking() {
        let now = instant("2026-01-01T00:00:00Z");
        for offset in [i64::MIN, i64::MAX, 9_000_000_000_000] {
            let p = VariableParams {
                offset: Some(offset),
                ..params("UTC")
            };
            assert!(validate(&p).is_err());
            assert!(render(&p, now).is_err());
        }
        for bad in ["%Q", "%", "%#z"] {
            let p = VariableParams {
                format: Some(bad.into()),
                ..params("UTC")
            };
            assert!(validate(&p).is_err());
            assert!(render(&p, now).is_err());
        }
        assert!(validate(&params("Mars/Olympus")).is_err());
        assert!(validate(&params("")).is_err());
    }
}
