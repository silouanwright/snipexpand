use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::{config::Config, expander::Expander};

#[derive(Debug, Serialize)]
pub struct Preview {
    pub trigger: String,
    pub source: PathBuf,
    pub profile: Option<String>,
    pub text: String,
    /// Unicode scalar values from the start of the rendered text.
    pub cursor_position: usize,
    /// Unicode scalar values from the end of the rendered text.
    pub cursor_back: usize,
}

pub fn render(
    config: &Config,
    trigger: &str,
    source: Option<&Path>,
    profile: Option<&str>,
) -> Result<Preview> {
    let profile_index = profile
        .map(|name| {
            config
                .settings
                .app_profiles
                .iter()
                .position(|p| p.name == name)
                .with_context(|| format!("unknown app profile '{name}'"))
        })
        .transpose()?;
    if let Some(index) = profile_index {
        if !config.settings.app_profiles[index].enabled {
            bail!(
                "app profile '{}' disables expansion",
                config.settings.app_profiles[index].name
            );
        }
    }
    let source = source
        .map(|path| {
            path.canonicalize()
                .with_context(|| format!("resolve source {}", path.display()))
        })
        .transpose()?;
    let matches = config.matches_for_profile(profile_index);
    let mut selected = Vec::new();
    for item in &matches {
        if item.regex.is_some() || !item.triggers.iter().any(|t| t == trigger) {
            continue;
        }
        if let Some(source) = &source {
            if item
                .source
                .canonicalize()
                .with_context(|| format!("resolve source {}", item.source.display()))?
                != *source
            {
                continue;
            }
        }
        selected.push(item);
    }
    if selected.is_empty() {
        bail!("literal trigger '{trigger}' not found in the active groups/selected source/profile; render does not evaluate regex triggers");
    }
    if selected.len() != 1 {
        bail!(
            "trigger '{trigger}' is ambiguous ({} matches); select --source or --profile",
            selected.len()
        );
    }
    let selected_source = selected[0].source.clone();
    let expander = Expander::new_configured(
        matches,
        config.settings.trigger_mode,
        config.settings.terminator_chars(),
        config.settings.word_separator_chars(),
        config.settings.regex_max_buffer,
    );
    let source_text = selected_source
        .to_str()
        .context("source path is not valid UTF-8")?;
    let expansion = expander
        .expansion_for_trigger(trigger, Some(source_text))?
        .context("selected snippet could not be rendered")?;
    Ok(Preview {
        trigger: trigger.into(),
        source: selected_source,
        profile: profile.map(str::to_owned),
        cursor_position: expansion.text.chars().count() - expansion.cursor_back,
        cursor_back: expansion.cursor_back,
        text: expansion.text,
    })
}
