//! Generated tests use a full input history model, independent of the rolling buffer.
use super::*;
use proptest::prelude::*;

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(vec!['a', 'b', 'é', '猫', '🦀']), 1..6)
        .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #[test]
    fn word_boundaries_preserve_separator(trigger in "[abé猫]{1,5}", prefix in prop::sample::select(vec!['a', '_', 'é', ' ', '.', '🙂'])) {
        let mut item = super::tests::structured_match(&trigger, "expanded");
        item.word = true;
        let mut engine = Expander::new(vec![item], TriggerMode::Immediate);
        for c in std::iter::once(prefix).chain(trigger.chars()) {
            prop_assert_eq!(engine.push_char(c), None);
        }
        let result = engine.push_char(' ');
        if prefix.is_alphanumeric() || prefix == '_' {
            prop_assert_eq!(result, None);
        } else {
            prop_assert_eq!(result, Some(Expansion {
                delete_count: trigger.chars().count() + 1,
                text: "expanded ".into(), cursor_back: 0,
                undo_text: format!("{trigger} "),
            }));
        }
    }

    #[test]
    fn regex_captures_match_only_the_suffix(digits in "[0-9]{1,12}", prefix in text()) {
        let mut item = super::tests::structured_match("", "id={{id}}");
        item.triggers.clear();
        item.regex = Some("issue-(?P<id>[0-9]+)".into());
        let mut engine = Expander::new(vec![item], TriggerMode::Space);
        for c in format!("{prefix}issue-{digits}").chars() {
            prop_assert_eq!(engine.push_char(c), None);
        }
        prop_assert_eq!(engine.push_char(' '), Some(Expansion {
            delete_count: 7 + digits.len(), text: format!("id={digits}"),
            cursor_back: 0, undo_text: format!("issue-{digits} "),
        }));
    }

    #[test]
    fn empty_matcher_retains_no_input(input in prop::collection::vec(any::<char>(), 1..400)) {
        let mut engine = Expander::new(Vec::<(String, String)>::new(), TriggerMode::Immediate);
        for c in input {
            prop_assert_eq!(engine.push_char(c), None);
            prop_assert!(engine.buffer.is_empty());
        }
    }

    #[test]
    fn literal_events_follow_suffix_model(
        triggers in prop::collection::vec(text(), 0..8),
        events in prop::collection::vec(0u8..10, 0..300),
        terminated in any::<bool>(),
    ) {
        let mode = if terminated { TriggerMode::Space } else { TriggerMode::Immediate };
        let mut engine = Expander::new_configured(
            triggers.iter().map(|t| (t.clone(), format!("<{t}>"))).collect::<Vec<_>>(),
            mode, vec![' ', '\t'], None, 256,
        );
        let longest = triggers.iter().map(|t| t.chars().count()).max().unwrap_or(0);
        let mut history = String::new();
        for event in events {
            match event {
                0 => { engine.reset(); history.clear(); }
                1 => { engine.pop_char(); history.pop(); }
                // A profile/reload transition must discard partial input.
                2 => {
                    engine.update_configured(triggers.iter().map(|t| (t.clone(), format!("<{t}>"))).collect::<Vec<_>>(), mode, vec![' ', '\t'], None, 256);
                    history.clear();
                }
                event => {
                    let c = ['a', 'b', 'é', '猫', '🦀', ' ', '\t'][(event - 3) as usize];
                    let terminator = terminated && matches!(c, ' ' | '\t');
                    if !terminator { history.push(c); }
                    let expected = if !terminated || terminator {
                        triggers.iter()
                            .filter(|t| triggers.iter().filter(|other| *other == *t).count() == 1)
                            .filter(|t| history.ends_with(t.as_str()))
                            .max_by_key(|t| t.chars().count())
                            .map(|t| Expansion {
                                delete_count: t.chars().count() + usize::from(terminator),
                                text: format!("<{t}>"), cursor_back: 0,
                                undo_text: if terminator { format!("{t}{c}") } else { t.clone() },
                            })
                    } else { None };
                    let actual = engine.push_char(c);
                    prop_assert_eq!(&actual, &expected);
                    if expected.is_some() || terminator { history.clear(); }
                    prop_assert!(engine.buffer.len() <= longest + 2 * usize::from(longest > 0));
                    // Forget history that the bounded engine can no longer know after backspace.
                    history = history.chars().rev().take(longest + 2 * usize::from(longest > 0))
                        .collect::<Vec<_>>().into_iter().rev().collect();
                }
            }
        }
    }

    #[test]
    fn cursor_metadata_counts_unicode_scalars(before in text(), after in text()) {
        let (output, back) = prepare_replacement(&format!("{before}$|${after}"));
        prop_assert_eq!(&output, &format!("{before}{after}"));
        prop_assert_eq!(back, after.chars().count());
        prop_assert!(back <= output.chars().count());
    }

    #[test]
    fn case_collisions_do_not_expand(trigger in "[a-z]{1,12}", reverse in any::<bool>()) {
        let mut entries = vec![(trigger.clone(), "first".into()), (trigger.to_uppercase(), "second".into())];
        if reverse { entries.reverse(); }
        let mut compiled = Vec::new();
        for entry in entries { entry.append_compiled(&mut compiled); }
        // A small adapter lets this property exercise the same public compilation path.
        struct Entry(CompiledMatch);
        impl IntoCompiledMatches for Entry {
            fn append_compiled(self, output: &mut Vec<CompiledMatch>) { output.push(self.0); }
        }
        compiled[0].propagate_case = true;
        let mut engine = Expander::new(compiled.into_iter().map(Entry).collect(), TriggerMode::Space);
        for c in trigger.chars() { prop_assert_eq!(engine.push_char(c), None); }
        prop_assert_eq!(engine.push_char(' '), None);
    }
}
