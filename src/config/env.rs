//! `${NAME}` substitution for configuration string values.
//!
//! Substitution runs on the parsed TOML tree, so comments and keys are never
//! touched. Only the `${NAME}` form is recognised; `$NAME` is left untouched
//! so that passwords containing `$` keep working. `$${` yields a literal
//! `${`. A reference to an unset variable is an error, which prevents a
//! literal `${MQTT_PASSWORD}` from being sent to the broker.

use std::fmt::Write;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EnvError {
    #[error("environment variable {0} is not set")]
    Unset(String),
    #[error("invalid variable reference at offset {0}: expected `${{NAME}}`")]
    Malformed(usize),
}

/// Expands every string value in `value`, recursively.
pub fn expand_value(
    value: &mut toml::Value,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<(), EnvError> {
    match value {
        toml::Value::String(s) => {
            if s.contains('$') {
                *s = expand(s, lookup)?;
            }
        }
        toml::Value::Array(items) => {
            for item in items {
                expand_value(item, lookup)?;
            }
        }
        toml::Value::Table(table) => {
            for (_, item) in table.iter_mut() {
                expand_value(item, lookup)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn expand(input: &str, lookup: impl Fn(&str) -> Option<String>) -> Result<String, EnvError> {
    if !input.contains('$') {
        return Ok(input.to_string());
    }
    let mut out = String::with_capacity(input.len() + 64);
    let mut rest = input;
    let mut offset = 0;
    while let Some(pos) = rest.find('$') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some(after) = tail.strip_prefix("$${") {
            out.push_str("${");
            rest = after;
            offset += pos + 3;
        } else if let Some(after) = tail.strip_prefix("${") {
            let end = after.find('}').ok_or(EnvError::Malformed(offset + pos))?;
            let name = &after[..end];
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(EnvError::Malformed(offset + pos));
            }
            let value = lookup(name).ok_or_else(|| EnvError::Unset(name.to_string()))?;
            write!(out, "{value}").expect("writing to String cannot fail");
            rest = &after[end + 1..];
            offset += pos + 2 + end + 1;
        } else {
            out.push('$');
            rest = &tail[1..];
            offset += pos + 1;
        }
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(name: &str) -> Option<String> {
        match name {
            "HOST" => Some("node1".into()),
            "EMPTY" => Some(String::new()),
            _ => None,
        }
    }

    #[test]
    fn substitutes_and_keeps_other_dollars() {
        assert_eq!(
            expand("a-${HOST}-b $HOST $ ${EMPTY}x", env).unwrap(),
            "a-node1-b $HOST $ x"
        );
        assert_eq!(expand("no refs", env).unwrap(), "no refs");
    }

    #[test]
    fn escapes_literal_reference() {
        assert_eq!(expand("$${HOST}", env).unwrap(), "${HOST}");
    }

    #[test]
    fn expands_strings_in_nested_tables_only() {
        let mut value: toml::Value = toml::from_str(
            "# comment with ${NOT_A_REF}\na = \"${HOST}\"\n[t]\nb = [\"x-${HOST}\", 1]\nc = 5\n",
        )
        .unwrap();
        expand_value(&mut value, &env).unwrap();
        assert_eq!(value["a"].as_str(), Some("node1"));
        assert_eq!(value["t"]["b"][0].as_str(), Some("x-node1"));
        assert_eq!(value["t"]["c"].as_integer(), Some(5));
    }

    #[test]
    fn reports_unset_and_malformed() {
        assert_eq!(expand("${NOPE}", env), Err(EnvError::Unset("NOPE".into())));
        assert_eq!(expand("x ${HOST", env), Err(EnvError::Malformed(2)));
        assert_eq!(expand("${BAD-NAME}", env), Err(EnvError::Malformed(0)));
        assert_eq!(expand("${}", env), Err(EnvError::Malformed(0)));
    }
}
