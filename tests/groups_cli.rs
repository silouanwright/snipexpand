use serde_json::{json, Value};
use std::process::{Command, Output};
use tempfile::TempDir;

struct Harness(TempDir);
impl Harness {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn dir(&self) -> std::path::PathBuf {
        self.0.path().join("config/snipexpand")
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.dir().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_snipexpand"));
        cmd.args(args)
            .env("XDG_CONFIG_HOME", self.0.path().join("config"))
            .env("XDG_DATA_HOME", self.0.path().join("data"))
            .env("XDG_RUNTIME_DIR", self.0.path().join("runtime"))
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY");
        cmd
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn success(&self, args: &[&str]) -> Vec<u8> {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }
    fn groups(&self) -> Value {
        serde_json::from_slice(&self.success(&["group", "list", "--json"])).unwrap()
    }
    fn setup(&self) {
        self.write("config.yml", "# retain this comment\nsnippet_groups:\n  - name: work\n    match_files: [work]\n  - name: shared\n    match_files: [work/signature.yml]\n    enabled: false\n");
        self.write(
            "match/work/signature.yml",
            "matches: [{trigger: ';sig', replace: 'Zoë'}]",
        );
        self.write("match/dependent.yml", "matches: [{trigger: ';dependent', replace: '{{sig}}', vars: [{name: sig, type: match, params: {trigger: ';sig'}}]}, {trigger: ';second', replace: '{{ref}}', vars: [{name: ref, type: match, params: {trigger: ';dependent'}}]}]");
        self.write(
            "match/plain.yml",
            "matches: [{trigger: ';plain', replace: plain}]",
        );
    }
}

#[test]
fn absent_config_listing_is_read_only_and_unknown_change_creates_nothing() {
    let h = Harness::new();
    assert_eq!(h.groups(), json!([]));
    assert!(!h.run(&["group", "toggle", "unknown"]).status.success());
    assert!(!h.0.path().join("config").exists());
}

#[test]
fn groups_persist_without_rewriting_yaml_and_suppress_dependencies_transitively() {
    let h = Harness::new();
    h.setup();
    let original = std::fs::read(h.dir().join("config.yml")).unwrap();
    assert_eq!(h.groups()[0]["members"], 1);
    assert_eq!(h.groups()[0]["available"], 0);
    for trigger in [";sig", ";dependent", ";second"] {
        assert!(!h.run(&["render", trigger]).status.success());
    }
    assert_eq!(h.success(&["render", ";plain"]), b"plain");
    h.success(&["group", "enable", "shared"]);
    for trigger in [";sig", ";dependent", ";second"] {
        assert_eq!(h.success(&["render", trigger]), "Zoë".as_bytes());
    }
    assert_eq!(h.groups()[1]["enabled"], true);
    assert_eq!(h.groups()[1]["default_enabled"], false);
    assert_eq!(h.groups()[1]["overridden"], true);
    h.success(&["group", "disable", "work"]);
    assert_eq!(h.groups()[0]["available"], 0);
    assert!(!h.dir().join("SKILL.md").exists());
    let entries: Value = serde_json::from_slice(&h.success(&["list", "--json"])).unwrap();
    let entry = entries
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["trigger"] == ";sig")
        .unwrap();
    assert_eq!(entry["groups"], json!(["work", "shared"]));
    assert_eq!(entry["available"], false);
    let source = h.dir().join("match/work/signature.yml");
    assert!(!h
        .run(&["render", ";sig", "--source", source.to_str().unwrap()])
        .status
        .success());
    h.success(&["group", "toggle", "work"]);
    assert_eq!(h.success(&["render", ";second"]), "Zoë".as_bytes());
    assert_eq!(std::fs::read(h.dir().join("config.yml")).unwrap(), original);
}

#[test]
fn profile_selection_intersects_groups_and_filters_missing_dependencies() {
    let h = Harness::new();
    h.setup();
    let mut settings = std::fs::read_to_string(h.dir().join("config.yml")).unwrap();
    settings.push_str("app_profiles:\n  - name: All\n    filter: {class: fake}\n  - name: Dependents\n    filter: {class: fake}\n    include_match_files: [dependent.yml]\n  - name: Off\n    filter: {class: fake}\n    enabled: false\n");
    h.write("config.yml", &settings);
    h.success(&["check"]);
    assert!(!h
        .run(&["render", ";sig", "--profile", "All"])
        .status
        .success());
    h.success(&["group", "enable", "shared"]);
    assert_eq!(
        h.success(&["render", ";second", "--profile", "All"]),
        "Zoë".as_bytes()
    );
    for profile in ["Dependents", "Off"] {
        assert!(!h
            .run(&["render", ";second", "--profile", profile])
            .status
            .success());
    }
}

#[test]
fn active_groups_disambiguate_plain_duplicates_and_use_root_relative_paths() {
    let h = Harness::new();
    h.write(
        "config.yml",
        "snippet_groups: [{name: one, match_files: [one.yml], enabled: false}]",
    );
    h.write(
        "match/one.yml",
        "matches: [{trigger: ';same', replace: one}]",
    );
    h.write(
        "match/nested/one.yml",
        "matches: [{trigger: ';same', replace: two}]",
    );
    assert_eq!(h.success(&["render", ";same"]), b"two");
    assert_eq!(h.groups()[0]["members"], 1);
    h.success(&["group", "enable", "one"]);
    assert!(!h.run(&["render", ";same"]).status.success());
}

#[test]
fn saved_unknown_names_are_retained_and_bad_state_is_rejected() {
    let h = Harness::new();
    h.setup();
    h.write("groups.json", r#"{"version":1,"enabled":{"future":false}}"#);
    h.success(&["group", "enable", "shared"]);
    let state: Value =
        serde_json::from_slice(&std::fs::read(h.dir().join("groups.json")).unwrap()).unwrap();
    assert_eq!(state["enabled"]["future"], false);
    let mut settings = std::fs::read_to_string(h.dir().join("config.yml")).unwrap();
    settings.push_str("  - name: future\n    match_files: [future]\n    enabled: true\n");
    h.write("config.yml", &settings);
    assert_eq!(h.groups()[2]["enabled"], false);
    for state in [
        "{",
        r#"{"version":2,"enabled":{}}"#,
        r#"{"version":1,"enabled":{"work":"no"}}"#,
        r#"{"version":1,"enabled":{},"extra":true}"#,
    ] {
        h.write("groups.json", state);
        assert!(!h.run(&["group", "toggle", "work"]).status.success());
        assert!(!h.run(&["check"]).status.success());
        assert_eq!(
            std::fs::read_to_string(h.dir().join("groups.json")).unwrap(),
            state
        );
    }
}

#[test]
fn concurrent_offline_toggles_do_not_lose_updates() {
    let h = Harness::new();
    h.setup();
    let mut children = Vec::new();
    for _ in 0..12 {
        children.push(
            h.command(&["group", "toggle", "work"])
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    assert_eq!(h.groups()[0]["enabled"], true);
}

#[test]
fn group_schema_and_parser_enforce_shape_names_and_path_rules() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/config.schema.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let h = Harness::new();
    for (group, schema_ok, parser_ok) in [
        (
            json!({"name":"work","match_files":["work","sig.yml"]}),
            true,
            true,
        ),
        (json!({"name":"work","match_files":[]}), false, false),
        (
            json!({"name":"bad name","match_files":["work"]}),
            false,
            false,
        ),
        (json!({"name":"work","match_files":[""]}), false, false),
        (
            json!({"name":"work","match_files":["../secret"]}),
            true,
            false,
        ),
        (json!({"name":"work","match_files":["."]}), true, false),
        (
            json!({"name":"work","match_files":["/absolute"]}),
            true,
            false,
        ),
        (json!({"name":"work","match_files":["packs"]}), true, false),
        (
            json!({"name":"work","match_files":["packs/test"]}),
            true,
            false,
        ),
        (
            json!({"name":"work","match_files":["work"],"enabled":"false"}),
            false,
            false,
        ),
        (
            json!({"name":"work","match_files":["work"],"unknown":true}),
            false,
            false,
        ),
    ] {
        let doc = json!({"snippet_groups":[group]});
        assert_eq!(validator.is_valid(&doc), schema_ok, "{doc}");
        h.write("config.yml", &doc.to_string());
        assert_eq!(h.run(&["check"]).status.success(), parser_ok, "{doc}");
    }
    h.write(
        "config.yml",
        "snippet_groups: [{name: same, match_files: [one]}, {name: same, match_files: [two]}]",
    );
    assert!(!h.run(&["check"]).status.success());
}

#[test]
fn dropped_ipc_response_never_falls_back_to_a_second_toggle() {
    use std::io::{BufRead, BufReader};
    let h = Harness::new();
    h.setup();
    let runtime = h.0.path().join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    let listener = std::os::unix::net::UnixListener::bind(runtime.join("snipexpand.sock")).unwrap();
    let state = h.dir().join("groups.json");
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        let request: Value =
            serde_json::from_str(line.trim().strip_prefix("group\t").unwrap()).unwrap();
        assert_eq!(request, json!({"operation":"toggle","name":"work"}));
        std::fs::write(state, r#"{"version":1,"enabled":{"work":false}}"#).unwrap();
        // Lost response after committing the toggle.
    });
    let output = h.run(&["group", "toggle", "work"]);
    server.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(h.groups()[0]["enabled"], false);
}
