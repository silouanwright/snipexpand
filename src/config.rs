use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerMode {
    #[default]
    Immediate,
    Space,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InjectionBackend {
    #[default]
    Auto,
    Wayland,
    Uinput,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonBmpInput {
    #[default]
    Auto,
    Keymap,
    Compose,
    InputMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Terminator {
    Space,
    Enter,
    Tab,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub trigger_mode: TriggerMode,
    #[serde(default = "default_terminators")]
    pub terminators: Vec<Terminator>,
    /// Optional Espanso-compatible characters that delimit words.
    #[serde(default)]
    pub word_separators: Option<Vec<String>>,
    /// Maximum rolling character buffer used by regex triggers.
    #[serde(default = "default_regex_max_buffer")]
    pub regex_max_buffer: usize,
    /// Injection transport. Auto prefers Wayland and falls back to uinput.
    #[serde(default)]
    pub injection_backend: InjectionBackend,
    /// How Wayland enters Unicode characters above U+FFFF.
    #[serde(default)]
    pub non_bmp_input: NonBmpInput,
    /// Milliseconds to pause after each injected key release.
    #[serde(default = "default_injection_delay_ms")]
    pub injection_delay_ms: u64,
    /// Optional Wayland-specific override for injection_delay_ms.
    #[serde(default)]
    pub wayland_injection_delay_ms: Option<u64>,
    /// Optional uinput-specific override for injection_delay_ms.
    #[serde(default)]
    pub uinput_injection_delay_ms: Option<u64>,
    /// One-time pause before deleting a matched trigger.
    #[serde(default = "default_injection_settle_ms")]
    pub injection_settle_ms: u64,
    /// Pause between keys in a numeric Unicode compose sequence.
    #[serde(default = "default_compose_delay_ms")]
    pub compose_delay_ms: u64,
    /// Pause before and after a numeric Unicode compose sequence.
    #[serde(default = "default_compose_settle_ms")]
    pub compose_settle_ms: u64,
    #[serde(default)]
    pub app_exclusions: Vec<AppFilter>,
    #[serde(default)]
    pub app_profiles: Vec<AppProfile>,
    #[serde(default = "default_true")]
    pub undo_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            trigger_mode: TriggerMode::Immediate,
            terminators: default_terminators(),
            word_separators: None,
            regex_max_buffer: default_regex_max_buffer(),
            injection_backend: InjectionBackend::Auto,
            non_bmp_input: NonBmpInput::Auto,
            injection_delay_ms: default_injection_delay_ms(),
            wayland_injection_delay_ms: None,
            uinput_injection_delay_ms: None,
            injection_settle_ms: default_injection_settle_ms(),
            compose_delay_ms: default_compose_delay_ms(),
            compose_settle_ms: default_compose_settle_ms(),
            app_exclusions: Vec::new(),
            app_profiles: Vec::new(),
            undo_enabled: true,
        }
    }
}

fn default_terminators() -> Vec<Terminator> {
    vec![Terminator::Space]
}

fn default_injection_delay_ms() -> u64 {
    1
}

fn default_regex_max_buffer() -> usize {
    256
}

fn default_injection_settle_ms() -> u64 {
    10
}

fn default_compose_delay_ms() -> u64 {
    5
}

fn default_compose_settle_ms() -> u64 {
    10
}

fn default_true() -> bool {
    true
}

fn validate_word_separators(values: Option<&[String]>) -> Result<()> {
    if let Some(values) = values {
        for value in values {
            if value.chars().count() != 1 {
                bail!("word_separators entries must contain exactly one character");
            }
        }
    }
    Ok(())
}

impl Settings {
    pub fn terminator_chars(&self) -> Vec<char> {
        self.terminators
            .iter()
            .map(|terminator| match terminator {
                Terminator::Space => ' ',
                Terminator::Enter => '\n',
                Terminator::Tab => '\t',
            })
            .collect()
    }

    pub fn word_separator_chars(&self) -> Option<Vec<char>> {
        self.word_separators.as_ref().map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .chars()
                        .next()
                        .expect("word separators were validated")
                })
                .collect()
        })
    }

    pub fn injection_delay_for(&self, backend: &str) -> u64 {
        match backend {
            "wayland" => self.wayland_injection_delay_ms,
            "uinput" => self.uinput_injection_delay_ms,
            _ => None,
        }
        .unwrap_or(self.injection_delay_ms)
    }

    pub fn requests_input_method(&self) -> bool {
        self.non_bmp_input == NonBmpInput::InputMethod
            || self.app_profiles.iter().any(|profile| {
                profile.enabled && profile.non_bmp_input == Some(NonBmpInput::InputMethod)
            })
    }

    pub fn profile_index(&self, app: &crate::app::AppInfo) -> Option<usize> {
        self.app_profiles
            .iter()
            .position(|profile| profile.matches(app))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppProfile {
    pub name: String,
    pub filter: AppFilter,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub include_match_files: Vec<String>,
    #[serde(default)]
    pub exclude_match_files: Vec<String>,
    #[serde(default)]
    pub trigger_mode: Option<TriggerMode>,
    #[serde(default)]
    pub terminators: Option<Vec<Terminator>>,
    #[serde(default)]
    pub word_separators: Option<Vec<String>>,
    #[serde(default)]
    pub injection_delay_ms: Option<u64>,
    #[serde(default)]
    pub injection_settle_ms: Option<u64>,
    #[serde(default)]
    pub compose_delay_ms: Option<u64>,
    #[serde(default)]
    pub compose_settle_ms: Option<u64>,
    #[serde(default)]
    pub non_bmp_input: Option<NonBmpInput>,
}

impl AppProfile {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("app profile name must not be empty");
        }
        self.filter.validate()?;
        for path in self
            .include_match_files
            .iter()
            .chain(&self.exclude_match_files)
        {
            let path = Path::new(path);
            if path.is_absolute()
                || path
                    .components()
                    .any(|part| part == std::path::Component::ParentDir)
            {
                bail!("app profile match files must be relative paths below match/");
            }
        }
        if self.injection_delay_ms.is_some_and(|value| value > 50) {
            bail!("app profile injection_delay_ms must be between 0 and 50");
        }
        if self.injection_settle_ms.is_some_and(|value| value > 100) {
            bail!("app profile injection_settle_ms must be between 0 and 100");
        }
        if self.compose_delay_ms.is_some_and(|value| value > 50) {
            bail!("app profile compose_delay_ms must be between 0 and 50");
        }
        if self.compose_settle_ms.is_some_and(|value| value > 100) {
            bail!("app profile compose_settle_ms must be between 0 and 100");
        }
        validate_word_separators(self.word_separators.as_deref())
    }

    pub fn matches(&self, app: &crate::app::AppInfo) -> bool {
        self.filter.matches(app)
    }

    fn includes_source(&self, source: &Path) -> bool {
        let included = self.include_match_files.is_empty()
            || self
                .include_match_files
                .iter()
                .any(|path| source.ends_with(path));
        included
            && !self
                .exclude_match_files
                .iter()
                .any(|path| source.ends_with(path))
    }
}

impl AppFilter {
    fn validate(&self) -> Result<()> {
        if self.title.is_none() && self.class.is_none() && self.exec.is_none() {
            bail!("app exclusion must specify at least one of title, class, or exec");
        }
        for (field, pattern) in [
            ("title", self.title.as_deref()),
            ("class", self.class.as_deref()),
            ("exec", self.exec.as_deref()),
        ] {
            if let Some(pattern) = pattern {
                regex::Regex::new(pattern)
                    .with_context(|| format!("invalid app exclusion {field} regex '{pattern}'"))?;
            }
        }
        Ok(())
    }

    fn matches(&self, app: &crate::app::AppInfo) -> bool {
        field_matches(self.title.as_deref(), app.title.as_deref())
            && field_matches(self.class.as_deref(), app.class.as_deref())
            && field_matches(self.exec.as_deref(), app.exec.as_deref())
    }
}

fn field_matches(pattern: Option<&str>, value: Option<&str>) -> bool {
    match (pattern, value) {
        (None, _) => true,
        (Some(pattern), Some(value)) => regex::Regex::new(pattern)
            .expect("app filter was validated")
            .is_match(value),
        (Some(_), None) => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub triggers: Vec<String>,
    pub regex: Option<String>,
    pub label: Option<String>,
    pub search_terms: Vec<String>,
    pub replace: String,
    pub vars: Vec<Variable>,
    pub word: bool,
    pub left_word: bool,
    pub right_word: bool,
    pub propagate_case: bool,
    pub uppercase_style: UppercaseStyle,
    pub source: PathBuf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UppercaseStyle {
    #[default]
    Uppercase,
    Capitalize,
    CapitalizeWords,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: VariableKind,
    #[serde(default)]
    pub params: VariableParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableKind {
    Date,
    Match,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariableParams {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub trigger: String,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub settings: Settings,
    pub matches: Vec<Match>,
    pub loaded_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreachableTrigger {
    pub trigger: String,
    pub source: PathBuf,
    pub blocking_trigger: String,
    pub blocking_source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateTrigger {
    pub trigger: String,
    pub sources: Vec<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct MatchFile {
    #[serde(default, rename = "name")]
    _package_name: Option<String>,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    global_vars: Vec<Variable>,
    #[serde(default)]
    matches: Vec<MatchDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MatchDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trigger: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    triggers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    regex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    search_terms: Vec<String>,
    replace: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    vars: Vec<Variable>,
    #[serde(default, skip_serializing_if = "is_false")]
    word: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    left_word: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    right_word: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    propagate_case: bool,
    #[serde(default, skip_serializing_if = "is_default_uppercase_style")]
    uppercase_style: UppercaseStyle,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    matches: Vec<MatchDefinition>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_default_uppercase_style(value: &UppercaseStyle) -> bool {
    *value == UppercaseStyle::Uppercase
}

impl Config {
    pub fn dir() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME not set");
                PathBuf::from(home).join(".config")
            });
        base.join("snipexpand")
    }

    pub fn match_dir() -> PathBuf {
        Self::dir().join("match")
    }

    pub fn generated_path() -> PathBuf {
        Self::match_dir().join("generated.yml")
    }

    pub fn load_default() -> Result<Self> {
        Self::load_dir(&Self::dir())
    }

    pub fn load_dir(dir: &Path) -> Result<Self> {
        let mut config = Self::default();
        let settings_path = dir.join("config.yml");
        if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)
                .with_context(|| format!("read {}", settings_path.display()))?;
            config.settings = parse_yaml(&content)
                .with_context(|| format!("parse {}", settings_path.display()))?;
            config.loaded_files.push(settings_path);
        }

        let match_dir = dir.join("match");
        if match_dir.exists() {
            let mut paths = yaml_files(&match_dir)?;
            paths.sort();
            for path in paths {
                config.load_match_file(&path)?;
            }
        }

        config.validate()?;
        Ok(config)
    }

    fn load_match_file(&mut self, path: &Path) -> Result<()> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let file: MatchFile =
            parse_yaml(&content).with_context(|| format!("parse {}", path.display()))?;
        if file
            .parent
            .as_deref()
            .is_some_and(|parent| parent != "default")
        {
            bail!(
                "{}: only the Espanso package parent 'default' is supported",
                path.display()
            );
        }
        for definition in file.matches {
            let triggers = definition.normalized_triggers(path)?;
            let vars = merge_variables(path, &file.global_vars, &definition.vars)?;
            self.matches.push(Match {
                triggers,
                regex: definition.regex,
                label: definition.label,
                search_terms: definition.search_terms,
                replace: definition.replace,
                vars,
                word: definition.word,
                left_word: definition.left_word,
                right_word: definition.right_word,
                propagate_case: definition.propagate_case,
                uppercase_style: definition.uppercase_style,
                source: path.to_path_buf(),
            });
        }
        self.loaded_files.push(path.to_path_buf());
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.settings.injection_delay_ms > 50 {
            bail!("injection_delay_ms must be between 0 and 50");
        }
        if self
            .settings
            .wayland_injection_delay_ms
            .is_some_and(|value| value > 50)
        {
            bail!("wayland_injection_delay_ms must be between 0 and 50");
        }
        if self
            .settings
            .uinput_injection_delay_ms
            .is_some_and(|value| value > 50)
        {
            bail!("uinput_injection_delay_ms must be between 0 and 50");
        }
        if self.settings.injection_settle_ms > 100 {
            bail!("injection_settle_ms must be between 0 and 100");
        }
        if self.settings.compose_delay_ms > 50 {
            bail!("compose_delay_ms must be between 0 and 50");
        }
        if self.settings.compose_settle_ms > 100 {
            bail!("compose_settle_ms must be between 0 and 100");
        }
        if !(32..=4096).contains(&self.settings.regex_max_buffer) {
            bail!("regex_max_buffer must be between 32 and 4096");
        }
        validate_word_separators(self.settings.word_separators.as_deref())?;
        for filter in &self.settings.app_exclusions {
            filter.validate()?;
        }
        let mut profile_names = HashSet::new();
        for profile in &self.settings.app_profiles {
            profile.validate()?;
            if !profile_names.insert(profile.name.as_str()) {
                bail!("duplicate app profile name '{}'", profile.name);
            }
        }
        for item in &self.matches {
            if !item.propagate_case && item.uppercase_style != UppercaseStyle::Uppercase {
                bail!(
                    "{}: uppercase_style requires propagate_case: true",
                    item.source.display()
                );
            }
            for trigger in &item.triggers {
                if trigger.is_empty() {
                    bail!("{}: trigger must not be empty", item.source.display());
                }
            }
            if let Some(pattern) = &item.regex {
                let regex = regex::Regex::new(&format!("(?:{pattern})$"))
                    .with_context(|| format!("{}: invalid regex", item.source.display()))?;
                if regex.is_match("") {
                    bail!("{}: regex must not match empty text", item.source.display());
                }
            }
            let mut names = HashSet::new();
            for var in &item.vars {
                if !names.insert(var.name.as_str()) {
                    bail!(
                        "{}: duplicate variable '{}'",
                        item.source.display(),
                        var.name
                    );
                }
                if var.kind == VariableKind::Match && var.params.trigger.is_empty() {
                    bail!(
                        "{}: match variable '{}' requires params.trigger",
                        item.source.display(),
                        var.name
                    );
                }
            }
        }
        self.validate_match_references()?;
        self.validate_profile_references()?;
        Ok(())
    }

    fn validate_match_references(&self) -> Result<()> {
        let by_trigger = self
            .matches
            .iter()
            .enumerate()
            .flat_map(|(index, item)| item.triggers.iter().map(move |trigger| (trigger, index)))
            .fold(
                HashMap::<&String, Vec<usize>>::new(),
                |mut map, (trigger, index)| {
                    map.entry(trigger).or_default().push(index);
                    map
                },
            );
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();
        for index in 0..self.matches.len() {
            validate_match_reference(index, self, &by_trigger, &mut visiting, &mut visited)?;
        }
        Ok(())
    }

    fn validate_profile_references(&self) -> Result<()> {
        for profile in &self.settings.app_profiles {
            for item in self
                .matches
                .iter()
                .filter(|item| profile.includes_source(&item.source))
            {
                for variable in item
                    .vars
                    .iter()
                    .filter(|variable| variable.kind == VariableKind::Match)
                {
                    let target = self
                        .matches
                        .iter()
                        .find(|candidate| candidate.triggers.contains(&variable.params.trigger))
                        .expect("nested match references were validated");
                    if !profile.includes_source(&target.source) {
                        bail!(
                            "app profile '{}': nested trigger '{}' is excluded",
                            profile.name,
                            variable.params.trigger
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn excludes_app(&self, app: &crate::app::AppInfo) -> bool {
        self.settings
            .app_exclusions
            .iter()
            .any(|filter| filter.matches(app))
    }

    pub fn matches_for_profile(&self, index: Option<usize>) -> Vec<Match> {
        let Some(profile) = index.and_then(|index| self.settings.app_profiles.get(index)) else {
            return self.matches.clone();
        };
        if !profile.enabled {
            return Vec::new();
        }
        self.matches
            .iter()
            .filter(|item| profile.includes_source(&item.source))
            .cloned()
            .collect()
    }

    /// Find longer triggers that immediate mode can never reach because a
    /// shorter trigger expands before the remaining characters are typed.
    pub fn unreachable_triggers(&self) -> Vec<UnreachableTrigger> {
        if self.settings.trigger_mode != TriggerMode::Immediate {
            return Vec::new();
        }
        let triggers = self
            .matches
            .iter()
            .flat_map(|item| {
                item.triggers
                    .iter()
                    .map(move |trigger| (trigger.as_str(), item))
            })
            .collect::<Vec<_>>();
        let mut unreachable = Vec::new();
        for (trigger, item) in &triggers {
            let blocker = triggers
                .iter()
                .filter(|(shorter, shorter_item)| {
                    shorter.len() < trigger.len()
                        && trigger.starts_with(shorter)
                        && !shorter_item.word
                        && !shorter_item.right_word
                })
                .min_by_key(|(shorter, _)| shorter.chars().count());
            if let Some((blocking_trigger, blocking_item)) = blocker {
                unreachable.push(UnreachableTrigger {
                    trigger: (*trigger).to_string(),
                    source: item.source.clone(),
                    blocking_trigger: (*blocking_trigger).to_string(),
                    blocking_source: blocking_item.source.clone(),
                });
            }
        }
        unreachable.sort_by(|left, right| left.trigger.cmp(&right.trigger));
        unreachable
    }

    pub fn duplicate_triggers(&self) -> Vec<DuplicateTrigger> {
        let mut grouped = HashMap::<&str, Vec<PathBuf>>::new();
        for item in &self.matches {
            for trigger in &item.triggers {
                grouped
                    .entry(trigger)
                    .or_default()
                    .push(item.source.clone());
            }
        }
        let mut duplicates = grouped
            .into_iter()
            .filter(|(_, sources)| sources.len() > 1)
            .map(|(trigger, sources)| DuplicateTrigger {
                trigger: trigger.to_string(),
                sources,
            })
            .collect::<Vec<_>>();
        duplicates.sort_by(|left, right| left.trigger.cmp(&right.trigger));
        duplicates
    }

    pub fn add_generated(
        trigger: &str,
        expansion: &str,
        label: Option<&str>,
        search_terms: &[String],
    ) -> Result<()> {
        if trigger.is_empty() {
            bail!("trigger must not be empty");
        }
        let path = Self::generated_path();
        let current = Self::load_default()?;
        if let Some(owner) = current
            .matches
            .iter()
            .find(|item| item.source != path && item.triggers.iter().any(|value| value == trigger))
        {
            bail!(
                "trigger '{trigger}' is owned by {}; edit that file directly",
                owner.source.display()
            );
        }
        let mut file = load_generated(&path)?;
        let previous_label = file
            .matches
            .iter()
            .find(|item| item.all_triggers().contains(&trigger))
            .and_then(|item| item.label.clone());
        let previous_search_terms = file
            .matches
            .iter()
            .find(|item| item.all_triggers().contains(&trigger))
            .map(|item| item.search_terms.clone())
            .unwrap_or_default();
        file.matches
            .retain(|item| !item.all_triggers().contains(&trigger));
        file.matches.push(MatchDefinition {
            trigger: Some(trigger.to_string()),
            triggers: Vec::new(),
            regex: None,
            label: updated_label(label, previous_label),
            search_terms: if search_terms.is_empty() {
                previous_search_terms
            } else {
                normalized_search_terms(search_terms)
            },
            replace: expansion.to_string(),
            vars: Vec::new(),
            word: false,
            left_word: false,
            right_word: false,
            propagate_case: false,
            uppercase_style: UppercaseStyle::Uppercase,
        });
        save_generated(&path, &file)
    }

    pub fn remove_generated(trigger: &str) -> Result<bool> {
        let path = Self::generated_path();
        let mut file = load_generated(&path)?;
        let before = file.matches.len();
        file.matches
            .retain(|item| !item.all_triggers().contains(&trigger));
        if file.matches.len() == before {
            return Ok(false);
        }
        save_generated(&path, &file)?;
        Ok(true)
    }
}

fn validate_match_reference(
    index: usize,
    config: &Config,
    by_trigger: &HashMap<&String, Vec<usize>>,
    visiting: &mut HashSet<usize>,
    visited: &mut HashSet<usize>,
) -> Result<()> {
    if visited.contains(&index) {
        return Ok(());
    }
    if !visiting.insert(index) {
        bail!(
            "{}: nested match reference cycle",
            config.matches[index].source.display()
        );
    }
    for variable in &config.matches[index].vars {
        if variable.kind != VariableKind::Match {
            continue;
        }
        let Some(targets) = by_trigger.get(&variable.params.trigger) else {
            bail!(
                "{}: match variable '{}' references unknown trigger '{}'",
                config.matches[index].source.display(),
                variable.name,
                variable.params.trigger
            );
        };
        if targets.len() != 1 {
            bail!(
                "{}: match variable '{}' references ambiguous trigger '{}'",
                config.matches[index].source.display(),
                variable.name,
                variable.params.trigger
            );
        }
        let target = targets[0];
        validate_match_reference(target, config, by_trigger, visiting, visited)?;
    }
    visiting.remove(&index);
    visited.insert(index);
    Ok(())
}

fn normalized_search_terms(values: &[String]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn updated_label(requested: Option<&str>, previous: Option<String>) -> Option<String> {
    match requested {
        Some(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        None => previous,
    }
}

impl MatchDefinition {
    fn all_triggers(&self) -> Vec<&str> {
        self.trigger
            .iter()
            .map(String::as_str)
            .chain(self.triggers.iter().map(String::as_str))
            .collect()
    }

    fn normalized_triggers(&self, path: &Path) -> Result<Vec<String>> {
        if self.regex.is_some() && (self.trigger.is_some() || !self.triggers.is_empty()) {
            bail!(
                "{}: a match must use either 'trigger', 'triggers', or 'regex'",
                path.display()
            );
        }
        if self.regex.as_deref().is_some_and(str::is_empty) {
            bail!("{}: regex must not be empty", path.display());
        }
        if self.regex.is_some() {
            return Ok(Vec::new());
        }
        match (&self.trigger, self.triggers.is_empty()) {
            (Some(_), false) => bail!(
                "{}: a match must use either 'trigger' or 'triggers', not both",
                path.display()
            ),
            (None, true) => bail!(
                "{}: a match requires 'trigger', 'triggers', or 'regex'",
                path.display()
            ),
            _ => Ok(self.all_triggers().into_iter().map(str::to_owned).collect()),
        }
    }
}

fn yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(yaml_files(&path)?);
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            files.push(path);
        }
    }
    Ok(files)
}

fn merge_variables(path: &Path, global: &[Variable], local: &[Variable]) -> Result<Vec<Variable>> {
    for variables in [global, local] {
        let mut names = HashSet::new();
        for variable in variables {
            if !names.insert(variable.name.as_str()) {
                bail!("{}: duplicate variable '{}'", path.display(), variable.name);
            }
        }
    }
    let local_names: HashSet<_> = local
        .iter()
        .map(|variable| variable.name.as_str())
        .collect();
    let mut merged: Vec<_> = global
        .iter()
        .filter(|variable| !local_names.contains(variable.name.as_str()))
        .cloned()
        .collect();
    merged.extend_from_slice(local);
    Ok(merged)
}

fn load_generated(path: &Path) -> Result<GeneratedFile> {
    match std::fs::read_to_string(path) {
        Ok(content) => parse_yaml(&content).with_context(|| format!("parse {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(GeneratedFile::default()),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

fn save_generated(path: &Path, file: &GeneratedFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_saphyr::to_string(file).context("serialize generated matches")?;
    let temporary = path.with_extension("yml.tmp");
    std::fs::write(&temporary, content)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub(crate) fn parse_yaml<T: DeserializeOwned>(
    content: &str,
) -> std::result::Result<T, serde_saphyr::Error> {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_aliases: 0,
            max_anchors: 0,
            max_documents: 1,
            max_depth: 32,
            max_events: 100_000,
            max_nodes: 25_000,
            max_total_scalar_bytes: 16 * 1024 * 1024,
            max_merge_keys: 0,
            max_inclusion_depth: 0,
        },
        merge_keys: serde_saphyr::MergeKeyPolicy::Error,
        strict_booleans: true,
    };
    serde_saphyr::from_str_with_options(content, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn loads_espanso_style_yaml_and_global_vars() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("match/personal.yml"),
            r#"
global_vars:
  - name: today
    type: date
    params:
      format: "%Y-%m-%d"
matches:
  - triggers: [";mail", ";email"]
    label: "Email address"
    search_terms: [email, contact]
    replace: "me@example.com"
  - trigger: ";today"
    replace: "{{today}}"
"#,
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert_eq!(config.matches.len(), 2);
        assert_eq!(config.matches[0].triggers, [";mail", ";email"]);
        assert_eq!(config.matches[0].label.as_deref(), Some("Email address"));
        assert_eq!(config.matches[0].search_terms, ["email", "contact"]);
        assert_eq!(config.matches[1].vars[0].name, "today");
    }

    #[test]
    fn rejects_unknown_espanso_features_instead_of_ignoring_them() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("match/forms.yml"),
            "matches:\n  - trigger: ';form'\n    form: 'Name: [[name]]'\n",
        );
        let error = Config::load_dir(dir.path()).unwrap_err().to_string();
        assert!(error.contains("forms.yml"));
    }

    #[test]
    fn rejects_anchors_and_multiple_documents() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("match/anchor.yml"),
            "matches:\n  - &shared\n    trigger: ';a'\n    replace: 'x'\n",
        );
        assert!(Config::load_dir(dir.path()).is_err());

        std::fs::remove_file(dir.path().join("match/anchor.yml")).unwrap();
        write(
            &dir.path().join("match/multiple.yml"),
            "matches: []\n---\nmatches: []\n",
        );
        assert!(Config::load_dir(dir.path()).is_err());
    }

    #[test]
    fn rejects_unsupported_variable_types() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("match/shell.yml"),
            "matches:\n  - trigger: ';cwd'\n    replace: '{{cwd}}'\n    vars:\n      - name: cwd\n        type: shell\n        params: {}\n",
        );
        let error = format!("{:#}", Config::load_dir(dir.path()).unwrap_err());
        assert!(error.contains("unknown variant") && error.contains("date"));
    }

    #[test]
    fn validates_nested_match_references_and_cycles() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("match/nested.yml");
        write(
            &path,
            "matches:\n  - trigger: ';name'\n    replace: 'Silouan'\n  - trigger: ';hello'\n    replace: 'Hello {{person}}'\n    vars:\n      - name: person\n        type: match\n        params:\n          trigger: ';name'\n",
        );
        Config::load_dir(dir.path()).unwrap();

        write(
            &path,
            "matches:\n  - trigger: ';a'\n    replace: '{{b}}'\n    vars:\n      - name: b\n        type: match\n        params: { trigger: ';b' }\n  - trigger: ';b'\n    replace: '{{a}}'\n    vars:\n      - name: a\n        type: match\n        params: { trigger: ';a' }\n",
        );
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("nested match reference cycle"));
    }

    #[test]
    fn loads_regex_triggers_and_rejects_invalid_patterns() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("match/regex.yml");
        write(
            &path,
            "matches:\n  - regex: 'issue (?P<number>\\d+)'\n    replace: 'Issue #{{number}}'\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert_eq!(
            config.matches[0].regex.as_deref(),
            Some(r"issue (?P<number>\d+)")
        );

        write(&path, "matches:\n  - regex: '[bad'\n    replace: 'x'\n");
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("invalid regex"));

        write(&path, "matches:\n  - regex: 'a*'\n    replace: 'x'\n");
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("must not match empty text"));
    }

    #[test]
    fn loads_case_propagation_and_rejects_ineffective_style() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("match/case.yml"),
            "matches:\n  - trigger: ';hello'\n    replace: 'good morning'\n    propagate_case: true\n    uppercase_style: capitalize_words\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert!(config.matches[0].propagate_case);
        assert_eq!(
            config.matches[0].uppercase_style,
            UppercaseStyle::CapitalizeWords
        );

        write(
            &dir.path().join("match/case.yml"),
            "matches:\n  - trigger: ';hello'\n    replace: 'good morning'\n    uppercase_style: capitalize\n",
        );
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("requires propagate_case: true"));
    }

    #[test]
    fn loads_and_validates_injection_timing() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("config.yml"),
            "injection_backend: wayland\nnon_bmp_input: compose\ninjection_delay_ms: 3\nwayland_injection_delay_ms: 0\nuinput_injection_delay_ms: 1\ninjection_settle_ms: 12\ncompose_delay_ms: 7\ncompose_settle_ms: 14\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert_eq!(config.settings.injection_backend, InjectionBackend::Wayland);
        assert_eq!(config.settings.non_bmp_input, NonBmpInput::Compose);
        assert_eq!(config.settings.injection_delay_ms, 3);
        assert_eq!(config.settings.injection_delay_for("wayland"), 0);
        assert_eq!(config.settings.injection_delay_for("uinput"), 1);
        assert_eq!(config.settings.injection_delay_for("other"), 3);
        assert_eq!(config.settings.injection_settle_ms, 12);
        assert_eq!(config.settings.compose_delay_ms, 7);
        assert_eq!(config.settings.compose_settle_ms, 14);

        write(&dir.path().join("config.yml"), "injection_delay_ms: 51\n");
        let error = Config::load_dir(dir.path()).unwrap_err().to_string();
        assert!(error.contains("injection_delay_ms must be between 0 and 50"));

        write(&dir.path().join("config.yml"), "compose_delay_ms: 51\n");
        let error = Config::load_dir(dir.path()).unwrap_err().to_string();
        assert!(error.contains("compose_delay_ms must be between 0 and 50"));

        write(&dir.path().join("config.yml"), "compose_settle_ms: 101\n");
        let error = Config::load_dir(dir.path()).unwrap_err().to_string();
        assert!(error.contains("compose_settle_ms must be between 0 and 100"));
    }

    #[test]
    fn input_method_is_opt_in_globally_or_by_profile() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join("config.yml"),
            "non_bmp_input: input_method\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert!(config.settings.requests_input_method());

        write(
            &dir.path().join("config.yml"),
            "app_profiles:\n  - name: Direct UTF-8\n    filter:\n      class: signal\n    non_bmp_input: input_method\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert!(config.settings.requests_input_method());

        write(
            &dir.path().join("config.yml"),
            "app_profiles:\n  - name: Disabled direct UTF-8\n    filter:\n      class: signal\n    enabled: false\n    non_bmp_input: input_method\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert!(!config.settings.requests_input_method());

        write(&dir.path().join("config.yml"), "non_bmp_input: auto\n");
        let config = Config::load_dir(dir.path()).unwrap();
        assert!(!config.settings.requests_input_method());
    }

    #[test]
    fn loads_and_validates_word_separators() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("config.yml"),
            "word_separators: [' ', '.', '🧐']\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        assert_eq!(
            config.settings.word_separator_chars(),
            Some(vec![' ', '.', '🧐'])
        );

        write(
            &dir.path().join("config.yml"),
            "word_separators: ['too long']\n",
        );
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("exactly one character"));
    }

    #[test]
    fn app_exclusions_match_all_fields_in_any_filter() {
        let mut config = Config::default();
        config.settings.app_exclusions = vec![
            AppFilter {
                class: Some("^1Password$".into()),
                ..Default::default()
            },
            AppFilter {
                title: Some("Secret".into()),
                exec: Some("/vault$".into()),
                ..Default::default()
            },
        ];
        config.validate().unwrap();

        assert!(config.excludes_app(&crate::app::AppInfo {
            class: Some("1Password".into()),
            ..Default::default()
        }));
        assert!(config.excludes_app(&crate::app::AppInfo {
            title: Some("Secret note".into()),
            exec: Some("/usr/bin/vault".into()),
            ..Default::default()
        }));
        assert!(!config.excludes_app(&crate::app::AppInfo {
            title: Some("Secret note".into()),
            exec: Some("/usr/bin/editor".into()),
            ..Default::default()
        }));
    }

    #[test]
    fn rejects_empty_or_invalid_app_exclusions() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("config.yml"), "app_exclusions:\n  - {}\n");
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("must specify at least one"));

        write(
            &dir.path().join("config.yml"),
            "app_exclusions:\n  - class: '[unterminated'\n",
        );
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("invalid app exclusion class regex"));
    }

    #[test]
    fn app_profiles_select_settings_and_match_files() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("config.yml"),
            "app_profiles:\n  - name: Browser\n    filter:\n      class: 'firefox'\n    include_match_files: [browser.yml]\n    trigger_mode: space\n    injection_delay_ms: 2\n    compose_delay_ms: 8\n    compose_settle_ms: 16\n    non_bmp_input: keymap\n",
        );
        write(
            &dir.path().join("match/browser.yml"),
            "matches:\n  - trigger: ';web'\n    replace: 'web'\n",
        );
        write(
            &dir.path().join("match/general.yml"),
            "matches:\n  - trigger: ';all'\n    replace: 'all'\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        let profile = config.settings.profile_index(&crate::app::AppInfo {
            class: Some("firefox".into()),
            ..Default::default()
        });
        assert_eq!(profile, Some(0));
        assert_eq!(
            config.settings.app_profiles[0].non_bmp_input,
            Some(NonBmpInput::Keymap)
        );
        assert_eq!(config.settings.app_profiles[0].compose_delay_ms, Some(8));
        assert_eq!(config.settings.app_profiles[0].compose_settle_ms, Some(16));
        assert_eq!(config.matches_for_profile(profile).len(), 1);
        assert_eq!(config.matches_for_profile(profile)[0].triggers, [";web"]);
    }

    #[test]
    fn app_profiles_reject_unsafe_paths() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("config.yml"),
            "app_profiles:\n  - name: Browser\n    filter: { class: firefox }\n    include_match_files: ['../secret.yml']\n",
        );
        assert!(Config::load_dir(dir.path())
            .unwrap_err()
            .to_string()
            .contains("relative paths"));
    }

    #[test]
    fn reports_duplicate_triggers_across_files() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("match/a.yml"),
            "matches:\n  - trigger: ';same'\n    replace: 'a'\n",
        );
        write(
            &dir.path().join("match/b.yml"),
            "matches:\n  - trigger: ';same'\n    replace: 'b'\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        let duplicates = config.duplicate_triggers();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].trigger, ";same");
        assert_eq!(duplicates[0].sources.len(), 2);
    }

    #[test]
    fn warns_about_unreachable_immediate_triggers() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("match/short.yml"),
            "matches:\n  - trigger: ';eur'\n    replace: '€'\n",
        );
        write(
            &dir.path().join("match/long.yml"),
            "matches:\n  - trigger: ';euro'\n    replace: '€'\n",
        );
        let config = Config::load_dir(dir.path()).unwrap();
        let warnings = config.unreachable_triggers();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].trigger, ";euro");
        assert_eq!(warnings[0].blocking_trigger, ";eur");

        write(&dir.path().join("config.yml"), "trigger_mode: space\n");
        assert!(Config::load_dir(dir.path())
            .unwrap()
            .unreachable_triggers()
            .is_empty());
    }

    #[test]
    fn generated_file_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("generated.yml");
        let file = GeneratedFile {
            matches: vec![MatchDefinition {
                trigger: Some(";sig".into()),
                triggers: Vec::new(),
                regex: None,
                label: Some("Signature".into()),
                search_terms: vec!["closing".into()],
                replace: "Best,\nSilouan".into(),
                vars: Vec::new(),
                word: false,
                left_word: false,
                right_word: false,
                propagate_case: false,
                uppercase_style: UppercaseStyle::Uppercase,
            }],
        };
        save_generated(&path, &file).unwrap();
        let loaded = load_generated(&path).unwrap();
        assert_eq!(loaded.matches[0].label.as_deref(), Some("Signature"));
        assert_eq!(loaded.matches[0].search_terms, ["closing"]);
        assert_eq!(loaded.matches[0].replace, "Best,\nSilouan");
    }

    #[test]
    fn generated_label_updates_preserve_set_and_clear_intentionally() {
        assert_eq!(
            updated_label(None, Some("Existing".into())).as_deref(),
            Some("Existing")
        );
        assert_eq!(
            updated_label(Some(" New label "), Some("Existing".into())).as_deref(),
            Some("New label")
        );
        assert_eq!(updated_label(Some(""), Some("Existing".into())), None);
    }
}
