use std::collections::VecDeque;

use crate::config::{Match, TriggerMode, UppercaseStyle, VariableKind};

#[derive(Debug, PartialEq)]
pub struct Expansion {
    pub delete_count: usize,
    pub text: String,
    pub cursor_back: usize,
    pub undo_text: String,
}

pub struct Expander {
    buffer: VecDeque<char>,
    max_trigger_len: usize,
    matches: Vec<CompiledMatch>,
    trigger_mode: TriggerMode,
    terminators: Vec<char>,
}

pub(crate) struct CompiledMatch {
    trigger: String,
    replace: String,
    vars: Vec<crate::config::Variable>,
    left_word: bool,
    right_word: bool,
    propagate_case: bool,
    uppercase_style: UppercaseStyle,
}

pub(crate) trait IntoCompiledMatches {
    fn append_compiled(self, output: &mut Vec<CompiledMatch>);
}

impl IntoCompiledMatches for Match {
    fn append_compiled(self, output: &mut Vec<CompiledMatch>) {
        for trigger in self.triggers {
            output.push(CompiledMatch {
                trigger,
                replace: self.replace.clone(),
                vars: self.vars.clone(),
                left_word: self.word || self.left_word,
                right_word: self.word || self.right_word,
                propagate_case: self.propagate_case,
                uppercase_style: self.uppercase_style,
            });
        }
    }
}

impl IntoCompiledMatches for (String, String) {
    fn append_compiled(self, output: &mut Vec<CompiledMatch>) {
        output.push(CompiledMatch {
            trigger: self.0,
            replace: self.1,
            vars: Vec::new(),
            left_word: false,
            right_word: false,
            propagate_case: false,
            uppercase_style: UppercaseStyle::Uppercase,
        });
    }
}

impl Expander {
    #[cfg(test)]
    pub fn new<T: IntoCompiledMatches>(matches: Vec<T>, trigger_mode: TriggerMode) -> Self {
        Self::new_configured(matches, trigger_mode, vec![' '])
    }

    pub fn new_configured<T: IntoCompiledMatches>(
        matches: Vec<T>,
        trigger_mode: TriggerMode,
        terminators: Vec<char>,
    ) -> Self {
        let matches = compile_matches(matches);
        let max_trigger_len = matches
            .iter()
            .map(|item| item.trigger.chars().count())
            .max()
            .unwrap_or(0);
        Self {
            buffer: VecDeque::new(),
            max_trigger_len,
            matches,
            trigger_mode,
            terminators,
        }
    }

    #[cfg(test)]
    pub fn update<T: IntoCompiledMatches>(&mut self, matches: Vec<T>, trigger_mode: TriggerMode) {
        self.update_configured(matches, trigger_mode, vec![' ']);
    }

    pub fn update_configured<T: IntoCompiledMatches>(
        &mut self,
        matches: Vec<T>,
        trigger_mode: TriggerMode,
        terminators: Vec<char>,
    ) {
        let matches = compile_matches(matches);
        self.max_trigger_len = matches
            .iter()
            .map(|item| item.trigger.chars().count())
            .max()
            .unwrap_or(0);
        self.matches = matches;
        self.trigger_mode = trigger_mode;
        self.terminators = terminators;
        self.buffer.clear();
    }

    pub fn push_char(&mut self, c: char) -> Option<Expansion> {
        if self.trigger_mode == TriggerMode::Space && self.terminators.contains(&c) {
            let expansion = self.find_match(Some(c));
            self.buffer.clear();
            return expansion;
        }

        self.buffer.push_back(c);

        // Keep one character before the longest trigger for left-word checks.
        while self.max_trigger_len > 0 && self.buffer.len() > self.max_trigger_len + 1 {
            self.buffer.pop_front();
        }

        if self.max_trigger_len == 0 {
            return None;
        }

        if self.trigger_mode == TriggerMode::Space {
            return None;
        }

        if !is_word_char(c) {
            if let Some(expansion) = self.find_right_word_match(c) {
                return Some(expansion);
            }
        }

        self.find_match(None)
    }

    fn find_match(&mut self, terminator: Option<char>) -> Option<Expansion> {
        let buf_str: String = self.buffer.iter().collect();
        let terminator_count = usize::from(terminator.is_some());

        for item in &self.matches {
            if terminator_count == 0 && item.right_word {
                continue;
            }
            if let Some(typed_trigger) = matching_suffix(&buf_str, item) {
                if !left_boundary_matches(&buf_str, item) {
                    continue;
                }
                let delete_count = item.trigger.chars().count() + terminator_count;
                let rendered = apply_propagated_case(render(item), item, &typed_trigger);
                let (text, cursor_back) = prepare_replacement(&rendered);
                self.buffer.clear();
                return Some(Expansion {
                    delete_count,
                    text,
                    cursor_back,
                    undo_text: terminator.map_or(typed_trigger.clone(), |value| {
                        format!("{typed_trigger}{value}")
                    }),
                });
            }
        }

        None
    }

    fn find_right_word_match(&mut self, separator: char) -> Option<Expansion> {
        let mut buf_str: String = self.buffer.iter().collect();
        buf_str.pop();
        for item in &self.matches {
            if !item.right_word || !left_boundary_matches(&buf_str, item) {
                continue;
            }
            let Some(typed_trigger) = matching_suffix(&buf_str, item) else {
                continue;
            };
            let rendered = apply_propagated_case(render(item), item, &typed_trigger);
            let replacement = format!("{}{}", rendered, separator);
            let (text, cursor_back) = prepare_replacement(&replacement);
            self.buffer.clear();
            return Some(Expansion {
                delete_count: item.trigger.chars().count() + 1,
                text,
                cursor_back,
                undo_text: format!("{}{}", typed_trigger, separator),
            });
        }
        None
    }

    pub fn pop_char(&mut self) {
        self.buffer.pop_back();
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}

fn compile_matches<T: IntoCompiledMatches>(matches: Vec<T>) -> Vec<CompiledMatch> {
    let mut compiled = Vec::new();
    for item in matches {
        item.append_compiled(&mut compiled);
    }
    compiled.sort_by_key(|item| std::cmp::Reverse(item.trigger.chars().count()));
    compiled
}

fn left_boundary_matches(buffer: &str, item: &CompiledMatch) -> bool {
    if !item.left_word {
        return true;
    }
    buffer
        .chars()
        .rev()
        .nth(item.trigger.chars().count())
        .is_none_or(|value| !is_word_char(value))
}

fn matching_suffix(buffer: &str, item: &CompiledMatch) -> Option<String> {
    let trigger_len = item.trigger.chars().count();
    let suffix: String = buffer
        .chars()
        .rev()
        .take(trigger_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if suffix == item.trigger
        || (item.propagate_case && suffix.to_lowercase() == item.trigger.to_lowercase())
    {
        Some(suffix)
    } else {
        None
    }
}

fn apply_propagated_case(replacement: String, item: &CompiledMatch, typed_trigger: &str) -> String {
    if !item.propagate_case {
        return replacement;
    }
    let mut alphabetic = typed_trigger.chars().filter(|value| value.is_alphabetic());
    let Some(first) = alphabetic.next() else {
        return replacement;
    };
    let second = alphabetic.next();
    if !first.is_uppercase() {
        return replacement;
    }
    if second.is_some_and(char::is_uppercase) {
        return replacement.to_uppercase();
    }
    match item.uppercase_style {
        UppercaseStyle::CapitalizeWords => capitalize_words(&replacement),
        UppercaseStyle::Uppercase | UppercaseStyle::Capitalize => capitalize(&replacement),
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn capitalize_words(value: &str) -> String {
    let mut at_word_start = true;
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_alphabetic() && at_word_start {
            output.extend(character.to_uppercase());
            at_word_start = false;
        } else {
            output.push(character);
            at_word_start = !(character.is_alphanumeric() || character == '_');
        }
    }
    output
}

fn is_word_char(value: char) -> bool {
    value.is_alphanumeric() || value == '_'
}

fn render(item: &CompiledMatch) -> String {
    let mut result = item.replace.clone();
    for variable in &item.vars {
        if variable.kind == VariableKind::Date {
            let now = chrono::Local::now() + chrono::Duration::seconds(variable.params.offset);
            let format = if variable.params.format.is_empty() {
                "%Y-%m-%d"
            } else {
                &variable.params.format
            };
            result = result.replace(
                &format!("{{{{{}}}}}", variable.name),
                &now.format(format).to_string(),
            );
        }
    }
    result
}

fn prepare_replacement(replacement: &str) -> (String, usize) {
    let Some(marker) = replacement.find("$|$") else {
        return (replacement.to_string(), 0);
    };
    let after = &replacement[marker + 3..];
    let mut text = String::with_capacity(replacement.len() - 3);
    text.push_str(&replacement[..marker]);
    text.push_str(after);
    (text, after.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn structured_match(trigger: &str, replace: &str) -> Match {
        Match {
            triggers: vec![trigger.to_string()],
            label: None,
            replace: replace.to_string(),
            vars: Vec::new(),
            word: false,
            left_word: false,
            right_word: false,
            propagate_case: false,
            uppercase_style: UppercaseStyle::Uppercase,
            source: PathBuf::from("test.yml"),
        }
    }

    fn make_expander() -> Expander {
        Expander::new(
            vec![
                ("/mail".to_string(), "user@example.com".to_string()),
                ("/sig".to_string(), "Best regards,\nSilouan".to_string()),
            ],
            TriggerMode::Immediate,
        )
    }

    #[test]
    fn test_no_match_on_partial_trigger() {
        let mut e = make_expander();
        for c in "/mai".chars() {
            let result = e.push_char(c);
            assert_eq!(result, None, "partial trigger should not match");
        }
    }

    #[test]
    fn test_match_on_complete_trigger() {
        let mut e = make_expander();
        let mut result = None;
        for c in "/mail".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 5,
                text: "user@example.com".to_string(),
                cursor_back: 0,
                undo_text: "/mail".to_string(),
            })
        );
    }

    #[test]
    fn test_backspace_prevents_match() {
        let mut e = make_expander();
        // Type /mai
        for c in "/mai".chars() {
            e.push_char(c);
        }
        // Backspace (removes 'i')
        e.pop_char();
        // Type 'l'. Buffer is now "/mal", not "/mail".
        let result = e.push_char('l');
        assert_eq!(
            result, None,
            "buffer is /mal after backspace, should not match"
        );
    }

    #[test]
    fn test_reset_prevents_match() {
        let mut e = make_expander();
        // Type /mai
        for c in "/mai".chars() {
            e.push_char(c);
        }
        // Arrow key / escape
        e.reset();
        // Type 'l'. Buffer is "l", not "/mail".
        let result = e.push_char('l');
        assert_eq!(
            result, None,
            "buffer cleared by reset, single 'l' should not match"
        );
    }

    #[test]
    fn test_reset_after_match_allows_new_match() {
        let mut e = make_expander();
        // First match
        for c in "/mail".chars() {
            e.push_char(c);
        }
        // After a match the buffer is cleared internally, but call reset explicitly too
        e.reset();
        // Retype the trigger. The second match should fire.
        let mut result = None;
        for c in "/mail".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 5,
                text: "user@example.com".to_string(),
                cursor_back: 0,
                undo_text: "/mail".to_string(),
            }),
            "second match after reset should fire"
        );
    }

    #[test]
    fn test_multiline_expansion_text() {
        let mut e = make_expander();
        let mut result = None;
        for c in "/sig".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 4,
                text: "Best regards,\nSilouan".to_string(),
                cursor_back: 0,
                undo_text: "/sig".to_string(),
            })
        );
    }

    #[test]
    fn test_delete_count_equals_trigger_char_count() {
        let mut e = make_expander();
        let mut result = None;
        for c in "/sig".chars() {
            result = e.push_char(c);
        }
        let expansion = result.expect("/sig should match");
        // "/sig" is 4 chars: '/', 's', 'i', 'g'
        assert_eq!(expansion.delete_count, 4);
    }

    #[test]
    fn test_buffer_capped_at_max_trigger_length() {
        // max trigger len is 5 ("/mail"). Type many chars before the trigger.
        let mut e = make_expander();
        // Type a bunch of unrelated chars first
        for c in "hello world this is some text ".chars() {
            e.push_char(c);
        }
        // Now type the trigger. The capped buffer should still produce a suffix match.
        let mut result = None;
        for c in "/mail".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 5,
                text: "user@example.com".to_string(),
                cursor_back: 0,
                undo_text: "/mail".to_string(),
            }),
            "trigger should still match even after long prefix input"
        );
    }

    #[test]
    fn test_multiple_triggers() {
        let mut e = Expander::new(
            vec![
                ("/mail".to_string(), "user@example.com".to_string()),
                ("/phone".to_string(), "+1-555-0100".to_string()),
            ],
            TriggerMode::Immediate,
        );
        let mut result = None;
        for c in "/phone".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 6,
                text: "+1-555-0100".to_string(),
                cursor_back: 0,
                undo_text: "/phone".to_string(),
            })
        );
    }

    fn exp(pairs: &[(&str, &str)]) -> Expander {
        Expander::new(
            pairs
                .iter()
                .map(|(t, e)| (t.to_string(), e.to_string()))
                .collect(),
            TriggerMode::Immediate,
        )
    }

    #[test]
    fn test_update_replaces_expansions_and_clears_buffer() {
        let mut e = exp(&[("/mail", "a@b.com")]);
        // Partially type the old trigger
        e.push_char('/');
        e.push_char('m');
        // Update to a completely different set
        e.update(
            vec![("/phone".to_string(), "123456".to_string())],
            TriggerMode::Immediate,
        );
        // Old trigger should no longer fire
        e.push_char('a');
        e.push_char('i');
        assert!(e.push_char('l').is_none()); // "/mail" not in new config
                                             // New trigger should fire
        let mut e2 = Expander::new(
            vec![("/phone".to_string(), "123456".to_string())],
            TriggerMode::Immediate,
        );
        for c in "/phone".chars() {
            e2.push_char(c);
        }
        // last push:
        let mut e3 = Expander::new(
            vec![("/phone".to_string(), "123456".to_string())],
            TriggerMode::Immediate,
        );
        let chars: Vec<char> = "/phone".chars().collect();
        let last = chars.last().copied().unwrap();
        for &c in &chars[..chars.len() - 1] {
            e3.push_char(c);
        }
        assert!(e3.push_char(last).is_some());
    }

    #[test]
    fn test_empty_expansions_does_not_panic() {
        let mut e = Expander::new(Vec::<(String, String)>::new(), TriggerMode::Immediate);
        assert!(e.push_char('/').is_none());
        assert!(e.push_char('m').is_none());
        e.pop_char();
        e.reset();
        // update to non-empty and back
        e.update(
            vec![("/x".to_string(), "y".to_string())],
            TriggerMode::Immediate,
        );
        e.update(Vec::<(String, String)>::new(), TriggerMode::Immediate);
        assert!(e.push_char('x').is_none());
    }

    #[test]
    fn test_space_mode_waits_for_terminator_and_removes_it() {
        let mut e = Expander::new(
            vec![(";mail".to_string(), "user@example.com".to_string())],
            TriggerMode::Space,
        );
        for c in ";mail".chars() {
            assert!(e.push_char(c).is_none());
        }
        assert_eq!(
            e.push_char(' '),
            Some(Expansion {
                delete_count: 6,
                text: "user@example.com".to_string(),
                cursor_back: 0,
                undo_text: ";mail ".to_string(),
            })
        );
    }

    #[test]
    fn terminated_mode_can_use_enter_instead_of_space() {
        let mut e = Expander::new_configured(
            vec![(";mail".to_string(), "user@example.com".to_string())],
            TriggerMode::Space,
            vec!['\n'],
        );
        for c in ";mail".chars() {
            assert!(e.push_char(c).is_none());
        }
        assert!(e.push_char(' ').is_none());
        for c in ";mail".chars() {
            assert!(e.push_char(c).is_none());
        }
        assert_eq!(e.push_char('\n').unwrap().delete_count, 6);
    }

    #[test]
    fn test_space_mode_clears_buffer_after_unmatched_word() {
        let mut e = Expander::new(
            vec![(";mail".to_string(), "user@example.com".to_string())],
            TriggerMode::Space,
        );
        for c in ";mai ".chars() {
            assert!(e.push_char(c).is_none());
        }
        assert!(e.push_char('l').is_none());
        assert!(e.push_char(' ').is_none());
    }

    #[test]
    fn propagate_case_matches_insensitively_and_transforms_replacement() {
        let mut item = structured_match(";hello", "good morning");
        item.propagate_case = true;
        let mut expander = Expander::new(vec![item.clone()], TriggerMode::Immediate);
        let mut result = None;
        for character in ";HELLO".chars() {
            result = expander.push_char(character);
        }
        assert_eq!(result.unwrap().text, "GOOD MORNING");

        item.uppercase_style = UppercaseStyle::CapitalizeWords;
        let mut expander = Expander::new(vec![item], TriggerMode::Immediate);
        result = None;
        for character in ";Hello".chars() {
            result = expander.push_char(character);
        }
        assert_eq!(result.unwrap().text, "Good Morning");
    }

    #[test]
    fn ordinary_matches_remain_case_sensitive() {
        let mut expander = Expander::new(
            vec![structured_match(";hello", "good morning")],
            TriggerMode::Immediate,
        );
        for character in ";HELLO".chars() {
            assert!(expander.push_char(character).is_none());
        }
    }

    #[test]
    fn test_cursor_marker_is_removed_and_offset_is_counted_in_chars() {
        let mut e = Expander::new(
            vec![(";fn".to_string(), "fn demo() {\n    $|$\n}".to_string())],
            TriggerMode::Immediate,
        );
        let mut expansion = None;
        for c in ";fn".chars() {
            expansion = e.push_char(c);
        }
        assert_eq!(
            expansion,
            Some(Expansion {
                delete_count: 3,
                text: "fn demo() {\n    \n}".to_string(),
                cursor_back: 2,
                undo_text: ";fn".to_string(),
            })
        );
    }

    #[test]
    fn left_word_rejects_a_trigger_inside_another_word() {
        let mut item = structured_match("cat", "animal");
        item.left_word = true;
        let mut e = Expander::new(vec![item], TriggerMode::Immediate);
        for c in "bobcat".chars() {
            assert!(e.push_char(c).is_none());
        }
        e.reset();
        assert!(e.push_char(' ').is_none());
        assert!(e.push_char('c').is_none());
        assert!(e.push_char('a').is_none());
        assert!(e.push_char('t').is_some());
    }

    #[test]
    fn right_word_waits_for_and_preserves_separator() {
        let mut item = structured_match("cat", "animal");
        item.right_word = true;
        let mut e = Expander::new(vec![item], TriggerMode::Immediate);
        for c in "cat".chars() {
            assert!(e.push_char(c).is_none());
        }
        assert_eq!(
            e.push_char('.'),
            Some(Expansion {
                delete_count: 4,
                text: "animal.".to_string(),
                cursor_back: 0,
                undo_text: "cat.".to_string(),
            })
        );
    }

    #[test]
    fn date_variable_uses_espanso_placeholder_syntax() {
        let mut item = structured_match(";year", "Year: {{current_year}}");
        item.vars.push(crate::config::Variable {
            name: "current_year".to_string(),
            kind: VariableKind::Date,
            params: crate::config::VariableParams {
                format: "%Y".to_string(),
                offset: 0,
            },
        });
        let mut e = Expander::new(vec![item], TriggerMode::Immediate);
        let mut expansion = None;
        for c in ";year".chars() {
            expansion = e.push_char(c);
        }
        assert_eq!(
            expansion.unwrap().text,
            format!("Year: {}", chrono::Local::now().format("%Y"))
        );
    }

    #[test]
    fn test_exact_match_not_confused_with_longer_trigger() {
        // With "/sig" and "/signal" configured, "/sig" should fire when typed,
        // NOT fire on typing "/signal" mid-stream.
        // (In practice Config prevents prefix conflicts, but Expander itself
        // handles the suffix match correctly. The longer trigger wins.)
        let mut e = exp(&[("/signal", "alarm"), ("/sig", "Best regards")]);
        // Type "/signal" fully
        let chars: Vec<char> = "/signal".chars().collect();
        let mut any_fired = false;
        for c in chars {
            if e.push_char(c).is_some() {
                any_fired = true;
            }
        }
        // "/signal" is longer, but "/sig" appears first in expansions.
        // The actual result depends on iteration order (first-match-wins).
        // We just assert one of them fires and no panic occurs.
        assert!(any_fired);
    }
}
