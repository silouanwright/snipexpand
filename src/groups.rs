//! Personal collection selection and serialized, atomic preference updates.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path};

use crate::config::Config;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    pub match_files: Vec<String>,
    #[serde(default = "enabled_default", deserialize_with = "strict_bool")]
    pub enabled: bool,
}
fn enabled_default() -> bool {
    true
}

fn strict_bool<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<bool, D::Error> {
    struct Bool;
    impl serde::de::Visitor<'_> for Bool {
        type Value = bool;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a boolean")
        }
        fn visit_bool<E: serde::de::Error>(self, value: bool) -> std::result::Result<bool, E> {
            Ok(value)
        }
    }
    deserializer.deserialize_any(Bool)
}

impl Group {
    pub fn contains(&self, relative: &Path) -> bool {
        !relative.starts_with("packs")
            && self
                .match_files
                .iter()
                .any(|path| relative.starts_with(path))
    }
}

pub fn validate(groups: &[Group]) -> Result<()> {
    let mut names = HashSet::new();
    for group in groups {
        if group.name.is_empty()
            || !group
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
        {
            bail!("group names must use ASCII letters, numbers, underscores, or hyphens");
        }
        if !names.insert(&group.name) {
            bail!("duplicate snippet group '{}'", group.name);
        }
        if group.match_files.is_empty() {
            bail!("group '{}' requires match_files", group.name);
        }
        for path in &group.match_files {
            let path = Path::new(path);
            if path.as_os_str().is_empty()
                || path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || path.starts_with("packs")
            {
                bail!(
                    "group '{}': match_files must be personal paths below match/, excluding packs/",
                    group.name
                );
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    version: u32,
    pub enabled: BTreeMap<String, bool>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: BTreeMap::new(),
        }
    }
}

pub fn read_state(dir: &Path) -> Result<State> {
    let path = dir.join("groups.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    };
    let state: State =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    if state.version != 1 {
        bail!("unsupported group state version {}", state.version);
    }
    Ok(state)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "lowercase", deny_unknown_fields)]
pub enum Request {
    List,
    Enable { name: String },
    Disable { name: String },
    Toggle { name: String },
}
impl Request {
    fn name(&self) -> Option<&str> {
        match self {
            Self::List => None,
            Self::Enable { name } | Self::Disable { name } | Self::Toggle { name } => Some(name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub enabled: bool,
    pub default_enabled: bool,
    pub overridden: bool,
    pub match_files: Vec<String>,
    /// All configured definitions in the collection, including inactive ones.
    pub members: usize,
    /// Available definitions with no app profile, after dependency filtering.
    pub available: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum Response {
    Ok { groups: Vec<Entry> },
    Error { error: String },
}

pub fn entries(config: &Config) -> Vec<Entry> {
    let active = config.active_match_indices(None);
    config
        .settings
        .snippet_groups
        .iter()
        .map(|group| {
            let indices: Vec<_> = config
                .matches
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    item.source
                        .strip_prefix(&config.match_root)
                        .is_ok_and(|path| group.contains(path))
                })
                .map(|(index, _)| index)
                .collect();
            Entry {
                name: group.name.clone(),
                enabled: config.group_enabled(group),
                default_enabled: group.enabled,
                overridden: config.group_state.enabled.contains_key(&group.name),
                match_files: group.match_files.clone(),
                members: indices.len(),
                available: indices
                    .iter()
                    .filter(|index| active.contains(index))
                    .count(),
            }
        })
        .collect()
}

pub fn change(dir: &Path, request: &Request) -> Result<Config> {
    change_with_writer(dir, request, write_state)
}

fn change_with_writer(
    dir: &Path,
    request: &Request,
    write: impl FnOnce(&Path, &State) -> Result<()>,
) -> Result<Config> {
    let name = request.name().context("list does not change group state")?;
    // Avoid creating configuration/lock files for an unknown group.
    let before = Config::load_dir(dir)?;
    if !before
        .settings
        .snippet_groups
        .iter()
        .any(|group| group.name == name)
    {
        bail!("unknown snippet group '{name}'");
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".groups.lock"))
        .context("open group state lock")?;
    let _lock = nix::fcntl::Flock::lock(lock, nix::fcntl::FlockArg::LockExclusive)
        .map_err(|(_, error)| anyhow::anyhow!("lock group state: {error}"))?;
    let mut candidate = Config::load_dir(dir)?;
    let group = candidate
        .settings
        .snippet_groups
        .iter()
        .find(|group| group.name == name)
        .with_context(|| format!("unknown snippet group '{name}'"))?;
    let enabled = match request {
        Request::Enable { .. } => true,
        Request::Disable { .. } => false,
        Request::Toggle { .. } => !candidate.group_enabled(group),
        Request::List => unreachable!(),
    };
    candidate.group_state.enabled.insert(name.into(), enabled);
    write(dir, &candidate.group_state)?;
    Ok(candidate)
}

fn write_state(dir: &Path, state: &State) -> Result<()> {
    write_state_with_commit(dir, state, |file, path| {
        file.persist(path)
            .context("replace group state atomically")?;
        Ok(())
    })
}

fn write_state_with_commit(
    dir: &Path,
    state: &State,
    commit: impl FnOnce(tempfile::NamedTempFile, &Path) -> Result<()>,
) -> Result<()> {
    let mut file =
        tempfile::NamedTempFile::new_in(dir).context("create group state temporary file")?;
    serde_json::to_writer_pretty(&mut file, state)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all().context("sync group state")?;
    commit(file, &dir.join("groups.json"))?;
    // Once renamed, the preference is committed; a directory-sync warning must
    // not encourage a caller to retry a toggle that already took effect.
    if let Err(error) = File::open(dir).and_then(|file| file.sync_all()) {
        tracing::warn!("Group state saved but directory sync failed: {error}");
    }
    Ok(())
}

/// Fall back only on a connection failure proving no request was sent.
pub fn command(dir: &Path, socket: Option<&Path>, request: &Request) -> Result<Vec<Entry>> {
    if let Some(socket) = socket {
        match std::os::unix::net::UnixStream::connect(socket) {
            Ok(mut stream) => {
                let timeout = Some(std::time::Duration::from_secs(5));
                stream.set_read_timeout(timeout)?;
                stream.set_write_timeout(timeout)?;
                writeln!(stream, "group\t{}", serde_json::to_string(request)?)?;
                stream.shutdown(std::net::Shutdown::Write)?;
                let mut response = String::new();
                BufReader::new(stream).read_line(&mut response).context("read group response; request may have been applied, inspect group list before retrying")?;
                return match serde_json::from_str(&response).context("invalid group response; request may have been applied, inspect group list before retrying")? {
                    Response::Ok { groups } => Ok(groups), Response::Error { error } => bail!("{error}"),
                };
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) => {}
            Err(error) => return Err(error).context("connect to daemon for group command"),
        }
    }
    let config = if matches!(request, Request::List) {
        Config::load_dir(dir)?
    } else {
        change(dir, request)?
    };
    Ok(entries(&config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.yml"),
            "snippet_groups: [{name: work, match_files: [work]}]",
        )
        .unwrap();
        dir
    }

    #[test]
    fn failed_atomic_commit_preserves_existing_state_and_removes_temp_file() {
        let dir = setup();
        change(
            dir.path(),
            &Request::Enable {
                name: "work".into(),
            },
        )
        .unwrap();
        let before = std::fs::read(dir.path().join("groups.json")).unwrap();
        let error = change_with_writer(
            dir.path(),
            &Request::Toggle {
                name: "work".into(),
            },
            |dir, state| {
                write_state_with_commit(dir, state, |_file, _destination| {
                    bail!("injected rename failure")
                })
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("injected rename failure"));
        assert_eq!(
            std::fs::read(dir.path().join("groups.json")).unwrap(),
            before
        );
        assert!(entries(&Config::load_dir(dir.path()).unwrap())[0].enabled);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[test]
    fn lock_failure_does_not_create_state() {
        let dir = setup();
        std::fs::create_dir(dir.path().join(".groups.lock")).unwrap();
        assert!(change(
            dir.path(),
            &Request::Disable {
                name: "work".into()
            }
        )
        .is_err());
        assert!(!dir.path().join("groups.json").exists());
    }
}
