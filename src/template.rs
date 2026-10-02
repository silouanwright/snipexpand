//! Single-pass interpolation: inserted values are data, never another template.
use anyhow::{bail, Context, Result};
use std::collections::HashMap;

use crate::config::{Variable, VariableKind};

pub const MAX_DEPTH: usize = 64;
pub const MAX_VARIABLES: usize = 4096;
pub const MAX_RENDER_BYTES: usize = 1024 * 1024;

pub enum Part<'a> {
    Text(&'a str),
    Reference(&'a str),
}

pub fn parts(input: &str) -> Vec<Part<'_>> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    while cursor < input.len() {
        let remaining = &input[cursor..];
        if remaining.starts_with(r"\{\{") {
            result.push(Part::Text(&input[start..cursor]));
            result.push(Part::Text("{{"));
            cursor += 4;
            start = cursor;
            continue;
        }
        if let Some(after) = remaining.strip_prefix("{{") {
            if let Some(end) = after.find("}}") {
                let name = &after[..end];
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    result.push(Part::Text(&input[start..cursor]));
                    result.push(Part::Reference(name));
                    cursor += end + 4;
                    start = cursor;
                    continue;
                }
            }
        }
        cursor += remaining.chars().next().map_or(1, char::len_utf8);
    }
    result.push(Part::Text(&input[start..]));
    result
}

pub fn references(input: &str) -> Vec<&str> {
    parts(input)
        .into_iter()
        .filter_map(|part| match part {
            Part::Reference(name) => Some(name),
            Part::Text(_) => None,
        })
        .collect()
}

pub struct Budget {
    bytes: usize,
    evaluations: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            bytes: MAX_RENDER_BYTES,
            evaluations: MAX_VARIABLES,
        }
    }
}

impl Budget {
    pub fn charge(&mut self, bytes: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .context("render exceeds the 1 MiB output budget")?;
        Ok(())
    }

    pub fn evaluate(&mut self) -> Result<()> {
        self.evaluations = self
            .evaluations
            .checked_sub(1)
            .context("render exceeds 4096 variable/match evaluations")?;
        Ok(())
    }

    pub fn interpolate(
        &mut self,
        input: &str,
        values: &HashMap<String, String>,
        strict: bool,
    ) -> Result<String> {
        if input.len() > MAX_RENDER_BYTES {
            bail!("template exceeds 1 MiB");
        }
        let mut output = String::new();
        for part in parts(input) {
            match part {
                Part::Text(text) => {
                    self.charge(text.len())?;
                    output.push_str(text);
                }
                Part::Reference(name) => {
                    if let Some(value) = values.get(name) {
                        self.charge(value.len())?;
                        output.push_str(value);
                    } else if strict {
                        bail!("unknown variable '{name}' in echo parameter");
                    } else {
                        self.charge(name.len() + 4)?;
                        output.push_str("{{");
                        output.push_str(name);
                        output.push_str("}}");
                    }
                }
            }
        }
        Ok(output)
    }
}

/// Dependencies precede their consumers, regardless of declaration order.
/// The depth guard precedes recursion, including when validating invalid input.
pub fn variable_order(vars: &[Variable], captures: &[&str]) -> Result<Vec<usize>> {
    if vars.len() > MAX_VARIABLES {
        bail!("at most 4096 variables are supported per snippet");
    }
    let indices: HashMap<_, _> = vars
        .iter()
        .enumerate()
        .map(|(i, v)| (v.name.as_str(), i))
        .collect();
    let mut state = vec![0; vars.len()];
    let mut heights = vec![0; vars.len()];
    let mut order = Vec::new();
    struct Visit<'a> {
        vars: &'a [Variable],
        captures: &'a [&'a str],
        indices: HashMap<&'a str, usize>,
        state: &'a mut [u8],
        heights: &'a mut [usize],
        order: &'a mut Vec<usize>,
    }
    impl Visit<'_> {
        fn run(&mut self, index: usize, depth: usize) -> Result<usize> {
            if depth > MAX_DEPTH {
                bail!("variable dependency depth exceeds {MAX_DEPTH}");
            }
            match self.state[index] {
                1 => bail!(
                    "variable dependency cycle involving '{}'",
                    self.vars[index].name
                ),
                2 => return Ok(self.heights[index]),
                _ => {}
            }
            self.state[index] = 1;
            let variable = &self.vars[index];
            let mut height = 1;
            if variable.kind == VariableKind::Echo && variable.inject_vars {
                for name in references(variable.params.echo.as_deref().unwrap_or_default()) {
                    if let Some(&target) = self.indices.get(name) {
                        height = height.max(1 + self.run(target, depth + 1)?);
                    } else if !self.captures.contains(&name) {
                        bail!(
                            "variable '{}': unknown variable '{name}' in echo parameter",
                            variable.name
                        );
                    }
                }
            }
            if height > MAX_DEPTH {
                bail!("variable dependency depth exceeds {MAX_DEPTH}");
            }
            self.state[index] = 2;
            self.heights[index] = height;
            self.order.push(index);
            Ok(height)
        }
    }
    let mut visit = Visit {
        vars,
        captures,
        indices,
        state: &mut state,
        heights: &mut heights,
        order: &mut order,
    };
    for index in 0..vars.len() {
        visit.run(index, 1)?;
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_is_single_pass_and_escape_is_literal() {
        let values = HashMap::from([
            ("one".into(), "{{two}}".into()),
            ("two".into(), "expanded".into()),
        ]);
        assert_eq!(
            Budget::default()
                .interpolate(r"{{one}} \{\{two}} {{two}} {{missing}}", &values, false)
                .unwrap(),
            "{{two}} {{two}} expanded {{missing}}"
        );
        assert!(Budget::default()
            .interpolate("{{missing}}", &values, true)
            .is_err());
    }

    #[test]
    fn output_budget_is_cumulative() {
        let mut budget = Budget::default();
        budget.charge(MAX_RENDER_BYTES - 1).unwrap();
        assert_eq!(
            budget
                .interpolate("é", &HashMap::new(), false)
                .unwrap_err()
                .to_string(),
            "render exceeds the 1 MiB output budget"
        );
    }
}
