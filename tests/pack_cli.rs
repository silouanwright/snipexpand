use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

struct Harness {
    _root: TempDir,
    repo: PathBuf,
    config_home: PathBuf,
    data_home: PathBuf,
}

impl Harness {
    fn native() -> Self {
        let root = tempfile::tempdir().unwrap();
        let harness = Self {
            repo: root.path().join("repo"),
            config_home: root.path().join("config"),
            data_home: root.path().join("data"),
            _root: root,
        };
        harness.write_native("0.1.0", "old expansion");
        harness.init_git();
        harness
    }

    fn write_native(&self, version: &str, replacement: &str) {
        write(
            &self.repo.join("pack.yml"),
            &format!(
                "name: test-pack\ntitle: Test Pack\nversion: {version}\ndescription: Test snippets\nauthor: Test\n"
            ),
        );
        write(
            &self.repo.join("match/pack.yml"),
            &format!("matches:\n  - trigger: ';pack'\n    replace: '{replacement}'\n"),
        );
    }

    fn init_git(&self) {
        git(&self.repo, &["init", "-q"]);
        self.commit("initial");
    }

    fn commit(&self, message: &str) {
        git(&self.repo, &["add", "."]);
        git(
            &self.repo,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                message,
            ],
        );
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_snipexpand"))
            .args(args)
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_DATA_HOME", &self.data_home)
            .output()
            .unwrap()
    }

    fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn mirror(&self) -> PathBuf {
        self.config_home
            .join("snipexpand/match/packs/test-pack/pack.yml")
    }
}

#[test]
fn pack_lifecycle_is_isolated_and_reversible() {
    let harness = Harness::native();
    let source = harness.repo.to_str().unwrap();

    let inspected = harness.success(&["pack", "inspect", source]);
    assert!(inspected.contains("Test Pack 0.1.0"));
    assert!(inspected.contains("snippets: 1"));

    harness.success(&["pack", "install", source]);
    assert!(harness.mirror().is_file());
    assert!(harness
        .success(&["list", "--json"])
        .contains("old expansion"));

    harness.success(&["pack", "disable", "test-pack"]);
    assert!(!harness.mirror().exists());
    harness.success(&["pack", "enable", "test-pack"]);
    assert!(harness.mirror().is_file());

    harness.success(&["pack", "remove", "test-pack"]);
    assert!(!harness.mirror().exists());
    assert_eq!(harness.success(&["pack", "list"]), "No packs installed.\n");
}

#[test]
fn pack_update_replaces_content_and_records_the_new_commit() {
    let harness = Harness::native();
    let source = harness.repo.to_str().unwrap();
    harness.success(&["pack", "install", source]);
    let before = pack_json(&harness)["commit"].as_str().unwrap().to_string();

    harness.write_native("0.2.0", "new expansion");
    harness.commit("update");
    harness.success(&["pack", "update", "test-pack"]);

    let after = pack_json(&harness);
    assert_eq!(after["version"], "0.2.0");
    assert_ne!(after["commit"].as_str().unwrap(), before);
    assert!(std::fs::read_to_string(harness.mirror())
        .unwrap()
        .contains("new expansion"));
}

#[test]
fn invalid_update_rolls_back_repository_state_and_active_matches() {
    let harness = Harness::native();
    let source = harness.repo.to_str().unwrap();
    harness.success(&["pack", "install", source]);
    let before = pack_json(&harness);

    write(
        &harness.repo.join("match/pack.yml"),
        "matches:\n  - trigger: ';pack'\n    shell: 'echo unsafe'\n",
    );
    harness.commit("invalid update");
    let output = harness.run(&["pack", "update", "test-pack"]);

    assert!(!output.status.success());
    assert_eq!(pack_json(&harness), before);
    assert!(std::fs::read_to_string(harness.mirror())
        .unwrap()
        .contains("old expansion"));
    assert!(harness.success(&["check"]).starts_with("OK:"));
}

#[test]
fn disabled_install_stays_out_of_the_active_configuration() {
    let harness = Harness::native();
    let source = harness.repo.to_str().unwrap();
    harness.success(&["pack", "install", source, "--disabled"]);

    assert!(!harness.mirror().exists());
    assert_eq!(pack_json(&harness)["enabled"], false);
    assert!(!harness.success(&["list", "--json"]).contains(";pack"));
}

#[test]
fn install_reports_trigger_conflicts_without_hiding_either_match() {
    let harness = Harness::native();
    write(
        &harness.config_home.join("snipexpand/match/personal.yml"),
        "matches:\n  - trigger: ';pack'\n    replace: 'personal expansion'\n",
    );
    let output = harness.success(&["pack", "install", harness.repo.to_str().unwrap()]);

    assert!(output.contains("WARNING: trigger ';pack' conflicts across 2 files"));
    let listed: Vec<Value> = serde_json::from_str(&harness.success(&["list", "--json"])).unwrap();
    assert_eq!(
        listed
            .iter()
            .filter(|entry| entry["trigger"] == ";pack")
            .count(),
        2
    );
}

fn pack_json(harness: &Harness) -> Value {
    let packs: Vec<Value> =
        serde_json::from_str(&harness.success(&["pack", "list", "--json"])).unwrap();
    packs.into_iter().next().unwrap()
}

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
