use crate::config::Config;
use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const ESPANSO_HUB_URL: &str = "https://github.com/espanso/hub";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    title: String,
    version: String,
    description: String,
    author: String,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    homepage: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    Native,
    Espanso,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledPack {
    name: String,
    title: String,
    version: String,
    description: String,
    author: String,
    source: String,
    #[serde(default)]
    subdir: Option<String>,
    #[serde(default)]
    git_ref: Option<String>,
    commit: String,
    format: Format,
    enabled: bool,
}

#[derive(Debug)]
struct InspectedPack {
    manifest: Manifest,
    format: Format,
    match_count: usize,
}

#[derive(Debug, Deserialize)]
struct MatchFileProbe {
    #[serde(default)]
    matches: Option<serde::de::IgnoredAny>,
    #[serde(default)]
    global_vars: Option<serde::de::IgnoredAny>,
}

pub fn inspect(source: &str, subdir: Option<&str>, git_ref: Option<&str>) -> Result<()> {
    let checkout = tempfile::tempdir().context("create temporary pack checkout")?;
    clone_repository(source, git_ref, checkout.path())?;
    let resolved_subdir = resolve_subdir(source, subdir, checkout.path())?;
    let pack = inspect_checkout(checkout.path(), resolved_subdir.as_deref())?;
    print_inspection(&pack, &git_commit(checkout.path())?);
    Ok(())
}

pub fn install(
    source: &str,
    subdir: Option<&str>,
    git_ref: Option<&str>,
    enabled: bool,
) -> Result<()> {
    validate_subdir(subdir)?;
    let root = packs_dir()?;
    std::fs::create_dir_all(&root)?;
    let staging = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(&root)
        .context("create pack staging directory")?;
    let repo = staging.path().join("repo");
    clone_repository(source, git_ref, &repo)?;
    let resolved_subdir = resolve_subdir(source, subdir, &repo)?;
    let inspected = inspect_checkout(&repo, resolved_subdir.as_deref())?;
    validate_name(&inspected.manifest.name)?;
    let destination = root.join(&inspected.manifest.name);
    if destination.exists() {
        bail!(
            "pack '{}' is already installed; use pack update",
            inspected.manifest.name
        );
    }

    let state = InstalledPack {
        name: inspected.manifest.name.clone(),
        title: inspected.manifest.title.clone(),
        version: inspected.manifest.version.clone(),
        description: inspected.manifest.description.clone(),
        author: inspected.manifest.author.clone(),
        source: source.to_string(),
        subdir: resolved_subdir,
        git_ref: git_ref.map(str::to_owned),
        commit: git_commit(&repo)?,
        format: inspected.format,
        enabled,
    };
    write_state(staging.path(), &state)?;
    let staging_path = staging.keep();
    std::fs::rename(&staging_path, &destination)?;

    if let Err(error) = apply_pack(&state) {
        let _ = remove_mirror(&state.name);
        let _ = std::fs::remove_dir_all(&destination);
        return Err(error);
    }
    signal_reload();
    println!(
        "Installed {} {}{}",
        state.title,
        state.version,
        if enabled { "" } else { " (disabled)" }
    );
    print_conflicts(&state.name)?;
    Ok(())
}

pub fn list(json: bool) -> Result<()> {
    let packs = installed_packs()?;
    if json {
        println!("{}", serde_json::to_string(&packs)?);
    } else if packs.is_empty() {
        println!("No packs installed.");
    } else {
        for pack in packs {
            println!(
                "{:<24} {:<10} {:<8} {}",
                pack.name,
                pack.version,
                if pack.enabled { "enabled" } else { "disabled" },
                short_commit(&pack.commit)
            );
        }
    }
    Ok(())
}

pub fn set_enabled(name: &str, enabled: bool) -> Result<()> {
    let mut state = load_named(name)?;
    if state.enabled == enabled {
        println!(
            "{} is already {}",
            state.name,
            if enabled { "enabled" } else { "disabled" }
        );
        return Ok(());
    }
    state.enabled = enabled;
    if enabled {
        apply_pack(&state)?;
    } else {
        let backup = deactivate_pack(&state.name)?;
        if let Err(error) = write_state(&packs_dir()?.join(name), &state) {
            restore_mirror(&state.name, backup.as_deref())?;
            return Err(error);
        }
        remove_backup(backup)?;
        signal_reload();
        println!("{} disabled", state.name);
        return Ok(());
    }
    if let Err(error) = write_state(&packs_dir()?.join(name), &state) {
        remove_mirror(&state.name)?;
        return Err(error);
    }
    signal_reload();
    println!(
        "{} {}",
        state.name,
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let state = load_named(name)?;
    let backup = deactivate_pack(name)?;
    let path = packs_dir()?.join(name);
    if let Err(error) = std::fs::remove_dir_all(&path) {
        restore_mirror(name, backup.as_deref())?;
        return Err(error).with_context(|| format!("remove {}", path.display()));
    }
    remove_backup(backup)?;
    signal_reload();
    println!("Removed {}", state.name);
    Ok(())
}

pub fn update(name: Option<&str>) -> Result<()> {
    let packs = match name {
        Some(name) => vec![load_named(name)?],
        None => installed_packs()?,
    };
    if packs.is_empty() {
        println!("No packs installed.");
        return Ok(());
    }
    for pack in packs {
        update_one(&pack)?;
    }
    signal_reload();
    Ok(())
}

fn update_one(current: &InstalledPack) -> Result<()> {
    let root = packs_dir()?;
    let staging = tempfile::Builder::new()
        .prefix(".update-")
        .tempdir_in(&root)
        .context("create pack update staging directory")?;
    let repo = staging.path().join("repo");
    clone_repository(&current.source, current.git_ref.as_deref(), &repo)?;
    let requested_subdir = if current.source.starts_with("espanso:") {
        None
    } else {
        current.subdir.as_deref()
    };
    let resolved_subdir = resolve_subdir(&current.source, requested_subdir, &repo)?;
    let inspected = inspect_checkout(&repo, resolved_subdir.as_deref())?;
    if inspected.manifest.name != current.name {
        bail!(
            "updated pack changed its name from '{}' to '{}'",
            current.name,
            inspected.manifest.name
        );
    }
    let mut updated = current.clone();
    updated.title = inspected.manifest.title;
    updated.version = inspected.manifest.version;
    updated.description = inspected.manifest.description;
    updated.author = inspected.manifest.author;
    updated.commit = git_commit(&repo)?;
    updated.format = inspected.format;
    updated.subdir = resolved_subdir;
    if updated.commit == current.commit {
        println!("{} is already up to date", current.name);
        return Ok(());
    }
    write_state(staging.path(), &updated)?;

    let destination = root.join(&current.name);
    let backup = root.join(format!(".backup-{}-{}", current.name, std::process::id()));
    let mirror = mirror_dir(&current.name);
    let mirror_backup = Config::dir().join(format!(
        ".pack-backup-{}-{}",
        current.name,
        std::process::id()
    ));
    if mirror.exists() {
        std::fs::rename(&mirror, &mirror_backup)?;
    }
    std::fs::rename(&destination, &backup)?;
    let staging_path = staging.keep();
    std::fs::rename(&staging_path, &destination)?;

    if let Err(error) = apply_pack(&updated) {
        let _ = std::fs::remove_dir_all(&destination);
        let _ = std::fs::rename(&backup, &destination);
        let _ = remove_mirror(&current.name);
        if mirror_backup.exists() {
            let _ = std::fs::rename(&mirror_backup, &mirror);
        }
        return Err(error);
    }
    if backup.exists() {
        std::fs::remove_dir_all(&backup)?;
    }
    if mirror_backup.exists() {
        std::fs::remove_dir_all(&mirror_backup)?;
    }
    println!(
        "Updated {} {} -> {}",
        current.name, current.version, updated.version
    );
    print_conflicts(&current.name)?;
    Ok(())
}

fn inspect_checkout(repo: &Path, subdir: Option<&str>) -> Result<InspectedPack> {
    validate_subdir(subdir)?;
    reject_symlinks(repo)?;
    let root = subdir.map_or_else(|| repo.to_path_buf(), |path| repo.join(path));
    if !root.is_dir() {
        bail!("pack directory does not exist: {}", root.display());
    }
    let (manifest_path, format) = if root.join("pack.yml").is_file() {
        (root.join("pack.yml"), Format::Native)
    } else if root.join("_manifest.yml").is_file() && root.join("package.yml").is_file() {
        (root.join("_manifest.yml"), Format::Espanso)
    } else {
        bail!(
            "{} is not a pack: expected pack.yml or _manifest.yml plus package.yml",
            root.display()
        );
    };
    let manifest: Manifest = parse_yaml_file(&manifest_path)?;
    validate_manifest(&manifest)?;
    let validation = tempfile::tempdir().context("create pack validation directory")?;
    let match_dir = validation.path().join("match");
    stage_matches(&root, format, &match_dir)?;
    let config = Config::load_dir(validation.path())?;
    if config.matches.is_empty() {
        bail!("pack '{}' contains no snippets", manifest.name);
    }
    Ok(InspectedPack {
        manifest,
        format,
        match_count: config.matches.len(),
    })
}

fn apply_pack(state: &InstalledPack) -> Result<()> {
    remove_mirror(&state.name)?;
    if state.enabled {
        let root = pack_content_root(state)?;
        stage_matches(&root, state.format, &mirror_dir(&state.name))?;
    }
    if let Err(error) = Config::load_default() {
        let _ = remove_mirror(&state.name);
        return Err(error).with_context(|| format!("enable pack '{}'", state.name));
    }
    Ok(())
}

fn deactivate_pack(name: &str) -> Result<Option<PathBuf>> {
    let mirror = mirror_dir(name);
    if !mirror.exists() {
        return Ok(None);
    }
    let backup = Config::dir().join(format!(".pack-backup-{}-{}", name, std::process::id()));
    if backup.exists() {
        bail!("pack backup already exists: {}", backup.display());
    }
    std::fs::rename(&mirror, &backup)?;
    if let Err(error) = Config::load_default() {
        std::fs::rename(&backup, &mirror)?;
        return Err(error).with_context(|| format!("disable pack '{name}'"));
    }
    Ok(Some(backup))
}

fn restore_mirror(name: &str, backup: Option<&Path>) -> Result<()> {
    if let Some(backup) = backup {
        std::fs::rename(backup, mirror_dir(name))?;
    }
    Ok(())
}

fn remove_backup(backup: Option<PathBuf>) -> Result<()> {
    if let Some(backup) = backup {
        std::fs::remove_dir_all(&backup)?;
    }
    Ok(())
}

fn pack_content_root(state: &InstalledPack) -> Result<PathBuf> {
    let repo = packs_dir()?.join(&state.name).join("repo");
    let root = state
        .subdir
        .as_deref()
        .map_or_else(|| repo.clone(), |path| repo.join(path));
    reject_symlinks(&root)?;
    Ok(root)
}

fn stage_matches(root: &Path, format: Format, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    match format {
        Format::Native => {
            let source = root.join("match");
            if !source.is_dir() {
                bail!("native pack requires a match directory");
            }
            copy_yaml_tree(&source, destination)?;
        }
        Format::Espanso => {
            copy_espanso_yaml(root, destination)?;
        }
    }
    Ok(())
}

fn copy_espanso_yaml(source: &Path, destination: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!(
                "pack contains unsupported symlink: {}",
                entry.path().display()
            );
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_espanso_yaml(&entry.path(), &target)?;
        } else if entry.file_name() != "_manifest.yml"
            && matches!(
                entry.path().extension().and_then(|value| value.to_str()),
                Some("yml" | "yaml")
            )
            && (entry.file_name() == "package.yml" || is_match_yaml(&entry.path())?)
        {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn is_match_yaml(path: &Path) -> Result<bool> {
    let probe: MatchFileProbe = parse_yaml_file(path)?;
    Ok(probe.matches.is_some() || probe.global_vars.is_some())
}

fn copy_yaml_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!(
                "pack contains unsupported symlink: {}",
                entry.path().display()
            );
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_yaml_tree(&entry.path(), &target)?;
        } else if matches!(
            entry.path().extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn reject_symlinks(root: &Path) -> Result<()> {
    let metadata =
        std::fs::symlink_metadata(root).with_context(|| format!("inspect {}", root.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("pack contains unsupported symlink: {}", root.display());
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(root)? {
            let path = entry?.path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            reject_symlinks(&path)?;
        }
    }
    Ok(())
}

fn clone_repository(source: &str, git_ref: Option<&str>, destination: &Path) -> Result<()> {
    if source.trim().is_empty() || source.starts_with('-') {
        bail!("invalid Git source");
    }
    if git_ref.is_some_and(|value| value.trim().is_empty() || value.starts_with('-')) {
        bail!("invalid Git ref");
    }
    if source.split_once("://").is_some_and(|(_, rest)| {
        rest.split('/')
            .next()
            .is_some_and(|host| host.contains('@'))
    }) {
        bail!("Git URLs must not contain credentials; use a credential helper or SSH");
    }
    if let Some(name) = source.strip_prefix("espanso:") {
        validate_name(name)?;
    }
    let git_source = source
        .strip_prefix("espanso:")
        .map_or(source, |_| ESPANSO_HUB_URL);
    run_git(
        &["clone", "--quiet", "--no-checkout", git_source],
        None,
        Some(destination),
    )?;
    run_git(
        &["checkout", "--quiet", git_ref.unwrap_or("HEAD"), "--"],
        Some(destination),
        None,
    )?;
    Ok(())
}

fn resolve_subdir(source: &str, subdir: Option<&str>, repo: &Path) -> Result<Option<String>> {
    let Some(name) = source.strip_prefix("espanso:") else {
        validate_subdir(subdir)?;
        return Ok(subdir.map(str::to_owned));
    };
    if subdir.is_some() {
        bail!("--path cannot be used with an espanso: pack source");
    }
    validate_name(name)?;
    let versions = repo.join("packages").join(name);
    let version = latest_version_dir(&versions)?
        .ok_or_else(|| anyhow::anyhow!("Espanso Hub pack '{name}' was not found"))?;
    Ok(Some(format!("packages/{name}/{version}")))
}

fn latest_version_dir(root: &Path) -> Result<Option<String>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("read {}", root.display())),
    };
    let mut versions = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let mut parts = name.split('.');
            let version = (
                parts.next()?.parse::<u64>().ok()?,
                parts.next()?.parse::<u64>().ok()?,
                parts.next()?.parse::<u64>().ok()?,
            );
            parts.next().is_none().then_some((version, name))
        })
        .collect::<Vec<_>>();
    versions.sort_by_key(|(version, _)| *version);
    Ok(versions.pop().map(|(_, name)| name))
}

fn git_commit(repo: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .context("run git rev-parse")?;
    if !output.status.success() {
        bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn run_git(args: &[&str], cwd: Option<&Path>, destination: Option<&Path>) -> Result<()> {
    let mut command = Command::new("git");
    command.args(["-c", "core.hooksPath=/dev/null"]).args(args);
    if let Some(path) = destination {
        command.arg(path);
    }
    if let Some(path) = cwd {
        command.current_dir(path);
    }
    let output = command.output().context("run git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn parse_yaml_file<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    crate::config::parse_yaml(&content).with_context(|| format!("parse {}", path.display()))
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    validate_name(&manifest.name)?;
    for (field, value) in [
        ("title", manifest.title.as_str()),
        ("version", manifest.version.as_str()),
        ("description", manifest.description.as_str()),
        ("author", manifest.author.as_str()),
    ] {
        if value.trim().is_empty() {
            bail!("pack {field} must not be empty");
        }
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        || name.starts_with('-')
        || name.ends_with('-')
    {
        bail!("pack name must use lowercase letters, numbers, and internal hyphens");
    }
    Ok(())
}

fn validate_subdir(subdir: Option<&str>) -> Result<()> {
    if let Some(value) = subdir {
        let path = Path::new(value);
        if value.is_empty()
            || path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            bail!("pack path must be a relative path inside the repository");
        }
    }
    Ok(())
}

fn installed_packs() -> Result<Vec<InstalledPack>> {
    let root = packs_dir()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut packs = std::fs::read_dir(&root)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|entry| load_state(&entry.path()).transpose())
        .collect::<Result<Vec<_>>>()?;
    packs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(packs)
}

fn load_named(name: &str) -> Result<InstalledPack> {
    validate_name(name)?;
    load_state(&packs_dir()?.join(name))?
        .ok_or_else(|| anyhow::anyhow!("pack '{name}' is not installed"))
}

fn load_state(dir: &Path) -> Result<Option<InstalledPack>> {
    let path = dir.join("state.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content)
            .with_context(|| format!("parse {}", path.display()))
            .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

fn write_state(dir: &Path, state: &InstalledPack) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("state.json");
    let temporary = dir.join("state.json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(state)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn packs_dir() -> Result<PathBuf> {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map_err(|_| anyhow::anyhow!("HOME environment variable is not set"))?;
    Ok(base.join("snipexpand/packs"))
}

fn mirror_dir(name: &str) -> PathBuf {
    Config::match_dir().join("packs").join(name)
}

fn remove_mirror(name: &str) -> Result<()> {
    let path = mirror_dir(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

fn print_inspection(pack: &InspectedPack, commit: &str) {
    println!("{} {}", pack.manifest.title, pack.manifest.version);
    println!("name: {}", pack.manifest.name);
    println!("author: {}", pack.manifest.author);
    println!("format: {:?}", pack.format);
    println!("snippets: {}", pack.match_count);
    println!("commit: {}", commit);
}

fn print_conflicts(name: &str) -> Result<()> {
    let config = Config::load_default()?;
    let mirror = mirror_dir(name);
    for duplicate in config.duplicate_triggers().into_iter().filter(|duplicate| {
        duplicate
            .sources
            .iter()
            .any(|source| source.starts_with(&mirror))
    }) {
        println!(
            "WARNING: trigger '{}' conflicts across {} files",
            duplicate.trigger,
            duplicate.sources.len()
        );
    }
    Ok(())
}

fn short_commit(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}

fn signal_reload() {
    let Ok(path) = crate::ipc::socket_path() else {
        return;
    };
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(path) else {
        return;
    };
    use std::io::Write;
    let _ = stream.write_all(b"reload\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_pack_names_and_subdirectories() {
        assert!(validate_name("useful-symbols").is_ok());
        assert!(validate_name("../escape").is_err());
        assert!(validate_name("Uppercase").is_err());
        assert!(validate_subdir(Some("packs/symbols")).is_ok());
        assert!(validate_subdir(Some("../symbols")).is_err());
        assert!(validate_subdir(Some("/tmp/symbols")).is_err());
    }

    #[test]
    fn selects_latest_stable_hub_version() {
        let root = tempfile::tempdir().unwrap();
        for version in ["0.9.0", "1.2.0", "1.10.0", "2.0.0-beta"] {
            std::fs::create_dir(root.path().join(version)).unwrap();
        }
        assert_eq!(
            latest_version_dir(root.path()).unwrap().as_deref(),
            Some("1.10.0")
        );
    }

    #[test]
    fn inspects_native_and_espanso_packs_strictly() {
        let native = tempfile::tempdir().unwrap();
        std::fs::write(
            native.path().join("pack.yml"),
            "name: symbols\ntitle: Symbols\nversion: 0.1.0\ndescription: Useful symbols\nauthor: Test\n",
        )
        .unwrap();
        std::fs::create_dir(native.path().join("match")).unwrap();
        std::fs::write(
            native.path().join("match/symbols.yml"),
            "matches:\n  - trigger: ';arrow'\n    replace: '→'\n",
        )
        .unwrap();
        let inspected = inspect_checkout(native.path(), None).unwrap();
        assert_eq!(inspected.format, Format::Native);
        assert_eq!(inspected.match_count, 1);

        let espanso = tempfile::tempdir().unwrap();
        std::fs::write(
            espanso.path().join("_manifest.yml"),
            "name: arrows\ntitle: Arrows\nversion: 0.1.0\ndescription: Useful arrows\nauthor: Test\n",
        )
        .unwrap();
        std::fs::write(
            espanso.path().join("package.yml"),
            "matches:\n  - trigger: ':arrow'\n    replace: '→'\n",
        )
        .unwrap();
        std::fs::write(
            espanso.path().join("extra.yml"),
            "matches:\n  - trigger: ':dash'\n    replace: '—'\n",
        )
        .unwrap();
        let inspected = inspect_checkout(espanso.path(), None).unwrap();
        assert_eq!(inspected.format, Format::Espanso);
        assert_eq!(inspected.match_count, 2);

        std::fs::write(
            espanso.path().join("package.yml"),
            "matches:\n  - trigger: ':unsafe'\n    shell: 'echo no'\n",
        )
        .unwrap();
        assert!(inspect_checkout(espanso.path(), None).is_err());
    }
}
