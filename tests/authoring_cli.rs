use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

struct Harness(TempDir);
impl Harness {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn write(&self, relative: &str, text: &str) {
        let path = self.0.path().join("config/snipexpand").join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_snipexpand"))
            .args(args)
            .current_dir(self.0.path())
            .env("XDG_CONFIG_HOME", self.0.path().join("config"))
            .env("XDG_DATA_HOME", self.0.path().join("data"))
            .env("XDG_RUNTIME_DIR", self.0.path().join("runtime"))
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .output()
            .unwrap()
    }
    fn success(&self, args: &[&str]) -> Vec<u8> {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
}

#[test]
fn preview_is_read_only_and_preserves_exact_text_and_cursor() {
    let h = Harness::new();
    assert!(!h.run(&["render", ";missing"]).status.success());
    h.success(&["check"]);
    assert!(!h.0.path().join("config").exists());
    h.write(
        "match/plain.yml",
        "matches:\n  - trigger: ';plain'\n    replace: 'α$|$🙂'\n",
    );
    assert_eq!(h.success(&["render", ";plain"]), "α🙂".as_bytes());
    let value: Value = serde_json::from_slice(&h.success(&["render", ";plain", "--json"])).unwrap();
    assert_eq!(value["text"], "α🙂");
    assert_eq!(value["cursor_position"], 1);
    assert_eq!(value["cursor_back"], 1);
    assert_eq!(value["profile"], Value::Null);
    assert!(value["source"].as_str().unwrap().ends_with("plain.yml"));
    assert!(!h.0.path().join("config/snipexpand/config.yml").exists());
    assert!(!h.0.path().join("config/snipexpand/SKILL.md").exists());
    assert!(!h.0.path().join("runtime").exists());
}

#[test]
fn preview_renders_nested_dates_but_does_not_simulate_typing() {
    let h = Harness::new();
    h.write(
        "match/examples.yml",
        include_str!("fixtures/authoring/matches.yml"),
    );
    let value: Value = serde_json::from_slice(&h.success(&["render", ";greet", "--json"])).unwrap();
    let year = chrono::Utc::now().format("%Y").to_string();
    assert_eq!(value["text"], format!("Hello World 🙂\n{year}"));
    assert_eq!(value["cursor_position"], 12);
    assert_eq!(
        h.success(&["render", ";hello"]),
        value["text"].as_str().unwrap().as_bytes()
    );
    let utc = String::from_utf8(h.success(&["render", ";utc"])).unwrap();
    let stamp = chrono::DateTime::parse_from_rfc3339(&utc).unwrap();
    assert_eq!(stamp.offset().local_minus_utc(), 0);
    assert!(
        (chrono::Utc::now() - stamp.with_timezone(&chrono::Utc))
            .num_seconds()
            .abs()
            < 5
    );
    let error = h.run(&["render", "issue-123"]);
    assert!(!error.status.success());
    assert!(String::from_utf8_lossy(&error.stderr).contains("does not evaluate regex"));
}

#[test]
fn duplicates_require_explicit_source_or_profile() {
    let h = Harness::new();
    h.write(
        "match/one.yml",
        "matches: [{trigger: ';same', replace: one}]",
    );
    h.write(
        "match/two.yml",
        "matches: [{trigger: ';same', replace: two}]",
    );
    h.write("config.yml", "app_profiles:\n  - name: One\n    filter: {class: never-focused}\n    include_match_files: [one.yml]\n  - name: Off\n    filter: {class: never-focused}\n    enabled: false\n");
    let error = h.run(&["render", ";same"]);
    assert!(!error.status.success());
    assert!(String::from_utf8_lossy(&error.stderr).contains("ambiguous"));
    assert_eq!(
        h.success(&[
            "render",
            ";same",
            "--source",
            "config/snipexpand/match/two.yml"
        ]),
        b"two"
    );
    assert_eq!(h.success(&["render", ";same", "--profile", "One"]), b"one");
    for args in [
        vec!["render", ";same", "--profile", "Unknown"],
        vec!["render", ";same", "--profile", "Off"],
        vec![
            "render",
            ";same",
            "--profile",
            "One",
            "--source",
            "config/snipexpand/match/two.yml",
        ],
    ] {
        assert!(!h.run(&args).status.success());
    }
    h.write(
        "match/two.yml",
        "matches: [{trigger: ';same', replace: two}, {trigger: ';same', replace: three}]",
    );
    assert!(!h
        .run(&[
            "render",
            ";same",
            "--source",
            "config/snipexpand/match/two.yml"
        ])
        .status
        .success());
}

fn schema(kind: &str) -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("schemas/{kind}.schema.json"));
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn check_case(kind: &str, doc: Value, expected_schema: bool, expected_parser: bool) {
    assert_eq!(
        schema(kind).is_valid(&doc),
        expected_schema,
        "schema mismatch for {doc}"
    );
    let h = Harness::new();
    h.write(
        if kind == "config" {
            "config.yml"
        } else {
            "match/test.yml"
        },
        &serde_json::to_string(&doc).unwrap(),
    );
    let output = h.run(&["check"]);
    assert_eq!(
        output.status.success(),
        expected_parser,
        "parser mismatch for {doc}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn documented_yaml_passes_schema_and_parser() {
    for (kind, text) in [
        ("config", include_str!("fixtures/authoring/config.yml")),
        ("match", include_str!("fixtures/authoring/matches.yml")),
    ] {
        let doc: Value = serde_saphyr::from_str(text).unwrap();
        check_case(kind, doc, true, true);
    }
}

#[test]
fn schemas_agree_with_parser_on_structure_and_ranges() {
    for doc in [
        json!({}),
        json!({"app_exclusions":[{"class":"x"}], "word_separators":["🙂"]}),
    ] {
        check_case("config", doc, true, true);
    }
    for doc in [
        json!({"nope":true}),
        json!({"injection_delay_ms":51}),
        json!({"injection_delay_ms":-1}),
        json!({"regex_max_buffer":31}),
        json!({"word_separators":["ab"]}),
        json!({"injection_backend":"clipboard"}),
        json!({"app_exclusions":[{}]}),
        json!({"app_profiles":[{"name":"x"}]}),
        json!({"app_profiles":[{"name":"  ","filter":{"class":"x"}}]}),
        json!({"app_profiles":[{"name":"x","filter":{"class":"x"},"injection_delay_ms":51}]}),
    ] {
        check_case("config", doc, false, false);
    }
    for item in [
        json!({"trigger":";x","replace":"hi","triggers":[]}),
        json!({"trigger":null,"regex":"abc","replace":"hi"}),
        json!({"trigger":";x","replace":"{{d}}","vars":[{"name":"d","type":"date"}]}),
        json!({"trigger":";x","replace":"{{d}}","vars":[{"name":"d","type":"date","params":{"format":null,"offset":null,"tz":null,"trigger":null}}]}),
    ] {
        check_case("match", json!({"matches":[item]}), true, true);
    }
    for item in [
        json!({"replace":"hi"}),
        json!({"trigger":"","replace":"hi"}),
        json!({"trigger":";x","triggers":[";y"],"replace":"hi"}),
        json!({"trigger":";x","regex":"abc","replace":"hi"}),
        json!({"trigger":";x","replace":"hi","shell":"echo hi"}),
        json!({"trigger":";x","replace":"hi","uppercase_style":"capitalize"}),
    ] {
        check_case("match", json!({"matches":[item]}), false, false);
    }
    for var in [
        json!({"name":"x","type":"shell","params":{"cmd":"echo hi"}}),
        json!({"name":"x","type":"date","params":{"trigger":";x"}}),
        json!({"name":"x","type":"match","params":{"trigger":";x","offset":0}}),
        json!({"name":"x","type":"match"}),
        json!({"name":"","type":"date"}),
        json!({"name":"x","type":"date","params":{"locale":"en-US"}}),
        json!({"name":"x","type":"date","params":{"tz":""}}),
    ] {
        check_case(
            "match",
            json!({"matches":[{"trigger":";x","replace":"{{x}}","vars":[var]}]}),
            false,
            false,
        );
    }
}

#[test]
fn check_handles_semantic_errors_beyond_editor_schema() {
    for params in [
        json!({"format":"%Q"}),
        json!({"format":"%#z"}),
        json!({"offset":i64::MAX}),
        json!({"offset":i64::MIN}),
        json!({"tz":"Mars/Olympus"}),
    ] {
        let doc = json!({"matches":[{"trigger":";x","replace":"{{x}}","vars":[{"name":"x","type":"date","params":params}]}]});
        check_case("match", doc, true, false);
    }
    // Invalid global definitions must be rejected even if unused or overridden.
    check_case(
        "match",
        json!({"global_vars":[{"name":"x","type":"date","params":{"format":"%Q"}}]}),
        true,
        false,
    );
    check_case(
        "match",
        json!({"matches":[{"trigger":";x","replace":"{{x}}","vars":[{"name":"x","type":"match","params":{"trigger":";x"}}]}]}),
        true,
        false,
    );
    check_case(
        "match",
        json!({"matches":[{"regex":"a*","replace":"hi"}]}),
        true,
        false,
    );
}

#[test]
fn schemas_can_be_exported_from_the_binary_without_setup() {
    let h = Harness::new();
    for kind in ["config", "match"] {
        let exported: Value = serde_json::from_slice(&h.success(&["schema", kind])).unwrap();
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("schemas/{kind}.schema.json"));
        let source: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(exported, source);
    }
    assert!(!h.run(&["schema", "unknown"]).status.success());
    assert!(!h.0.path().join("config").exists());
}
