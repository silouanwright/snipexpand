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

#[test]
fn echo_dependencies_overrides_escaping_and_nested_matches() {
    let text = include_str!("fixtures/authoring/echo.yml");
    check_case("match", serde_saphyr::from_str(text).unwrap(), true, true);
    let h = Harness::new();
    h.write("match/echo.yml", text);
    assert_eq!(
        h.success(&["render", ";signature"]),
        "Zoë 🦀 — Engineer".as_bytes()
    );
    assert_eq!(
        h.success(&["render", ";literal"]),
        b"{{name}} | {{name}} | Ada"
    );
    assert_eq!(
        h.success(&["render", ";nested-echo"]),
        "Hello Zoë 🦀 — Engineer".as_bytes()
    );
}

#[test]
fn echo_validation_rejects_unsupported_parameters_and_dependency_errors() {
    for variable in [
        json!({"name":"x","type":"echo"}),
        json!({"name":"x","type":"echo","params":{"echo":null}}),
        json!({"name":"x","type":"echo","params":{"echo":"yes","offset":1}}),
        json!({"name":"x","type":"date","params":{"echo":"yes"}}),
        json!({"name":"x","type":"echo","depends_on":["y"],"params":{"echo":"yes"}}),
        json!({"name":"x","type":"date","inject_vars":false}),
    ] {
        check_case(
            "match",
            json!({"matches":[{"trigger":";test","replace":"{{x}}","vars":[variable]}]}),
            false,
            false,
        );
    }
    for echo in ["{{missing}}", "{{x}}"] {
        check_case(
            "match",
            json!({"matches":[{"trigger":";test","replace":"{{x}}","vars":[{"name":"x","type":"echo","params":{"echo":echo}}]}]}),
            true,
            false,
        );
    }
    check_case(
        "match",
        json!({"matches":[{"trigger":";test","replace":"{{x}}","vars":[{"name":"x","type":"date","params":{"format":"{{missing}}"}}]}]}),
        true,
        false,
    );
}

#[test]
fn echo_output_growth_fails_before_preview_writes_output() {
    let h = Harness::new();
    let mut vars = vec![json!({"name":"v0","type":"echo","params":{"echo":"x"}})];
    for index in 1..24 {
        let prev = index - 1;
        vars.push(json!({"name":format!("v{index}"),"type":"echo","params":{"echo":format!("{{{{v{prev}}}}}{{{{v{prev}}}}}")}}));
    }
    h.write(
        "match/growth.yml",
        &json!({"matches":[{"trigger":";growth","replace":"{{v23}}","vars":vars}]}).to_string(),
    );
    h.success(&["check"]);
    let output = h.run(&["render", ";growth"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output budget"));
}

mod generated {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn echo_graph_schema_parser_and_output_agree(count in 1usize..12, branching in any::<bool>(), reverse in any::<bool>(), leaf in "[a-zé猫]{1,8}") {
            let mut vars = vec![json!({"name":"v0","type":"echo","params":{"echo":leaf}})];
            for index in 1..count {
                let prev = index - 1;
                let reference = format!("{{{{v{prev}}}}}");
                vars.push(json!({"name":format!("v{index}"),"type":"echo","params":{"echo":if branching { reference.repeat(2) } else { reference }}}));
            }
            if reverse { vars.reverse(); }
            let doc = json!({"matches":[{"trigger":";graph","replace":format!("{{{{v{}}}}}", count - 1),"vars":vars}]});
            prop_assert!(schema("match").is_valid(&doc));
            let h = Harness::new();
            h.write("match/graph.yml", &doc.to_string());
            let expected = leaf.repeat(if branching { 1 << (count - 1) } else { 1 });
            prop_assert_eq!(h.success(&["render", ";graph"]), expected.as_bytes());
        }

        #[test]
        fn nested_match_graphs_reject_cycles_missing_targets_and_depth(count in 1usize..75, error in 0u8..3, reverse in any::<bool>()) {
            let mut entries = Vec::new();
            for index in 0..count {
                let target = if index + 1 < count { Some(index + 1) } else { match error { 1 => Some(0), 2 => Some(count), _ => None } };
                entries.push(match target {
                    Some(target) => json!({"trigger":format!(";g{index}"),"replace":"{{nested}}","vars":[{"name":"nested","type":"match","params":{"trigger":format!(";g{target}")}}]}),
                    None => json!({"trigger":format!(";g{index}"),"replace":"leaf"}),
                });
            }
            if reverse { entries.reverse(); }
            let h = Harness::new();
            h.write("match/graph.yml", &json!({"matches":entries}).to_string());
            let output = h.run(&["render", ";g0"]);
            prop_assert_eq!(output.status.success(), error == 0 && count <= 64);
            if output.status.success() { prop_assert_eq!(output.stdout, b"leaf"); }
            else { prop_assert!(output.stdout.is_empty()); }
        }

        #[test]
        fn echo_graphs_reject_cycles_missing_names_and_depth(count in 1usize..75, error in 0u8..3, reverse in any::<bool>()) {
            let mut vars = Vec::new();
            for index in 0..count {
                let target = if index + 1 < count { Some(index + 1) } else { match error { 1 => Some(0), 2 => Some(count), _ => None } };
                vars.push(json!({"name":format!("v{index}"),"type":"echo","params":{"echo":target.map_or_else(|| "leaf".to_string(), |t| format!("{{{{v{t}}}}}"))}}));
            }
            if reverse { vars.reverse(); }
            let h = Harness::new();
            h.write("match/graph.yml", &json!({"matches":[{"trigger":";graph","replace":"{{v0}}","vars":vars}]}).to_string());
            let output = h.run(&["render", ";graph"]);
            prop_assert_eq!(output.status.success(), error == 0 && count <= 64);
            if output.status.success() { prop_assert_eq!(output.stdout, b"leaf"); }
            else { prop_assert!(output.stdout.is_empty()); }
        }
    }
}

#[test]
fn branching_nested_matches_stop_at_evaluation_budget() {
    let h = Harness::new();
    let mut entries = vec![json!({"trigger":";g0","replace":""})];
    for index in 1..20 {
        let target = format!(";g{}", index - 1);
        entries.push(
            json!({"trigger":format!(";g{index}"),"replace":"{{a}}{{b}}","vars":[
                {"name":"a","type":"match","params":{"trigger":target}},
                {"name":"b","type":"match","params":{"trigger":target}},
            ]}),
        );
    }
    h.write("match/branch.yml", &json!({"matches":entries}).to_string());
    h.success(&["check"]);
    let output = h.run(&["render", ";g19"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("4096 variable/match evaluations"));
}
