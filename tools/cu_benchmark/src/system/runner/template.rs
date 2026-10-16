use super::Variables;
use anyhow::{ensure, Context, Result};
use serde_json::Value;

pub fn render(value: &Value, variables: &Variables) -> Result<Value> {
    match value {
        Value::String(text) => {
            if text.starts_with("${")
                && text.ends_with('}')
                && !text[2..text.len() - 1].contains("${")
            {
                return lookup(&text[2..text.len() - 1], variables);
            }
            let mut result = String::new();
            let mut rest = text.as_str();
            while let Some(index) = rest.find("${") {
                result.push_str(&rest[..index]);
                let end = rest[index + 2..].find('}').context("unclosed variable")? + index + 2;
                let value = lookup(&rest[index + 2..end], variables)?;
                result.push_str(
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                        .as_str(),
                );
                rest = &rest[end + 1..];
            }
            result.push_str(rest);
            Ok(Value::String(result))
        }
        Value::Array(values) => Ok(Value::Array(
            values
                .iter()
                .map(|v| render(v, variables))
                .collect::<Result<_>>()?,
        )),
        Value::Object(values) => Ok(Value::Object(
            values
                .iter()
                .map(|(k, v)| Ok((k.clone(), render(v, variables)?)))
                .collect::<Result<_>>()?,
        )),
        _ => Ok(value.clone()),
    }
}

fn lookup(key: &str, variables: &Variables) -> Result<Value> {
    if let Some(name) = key.strip_prefix("env:") {
        return Ok(Value::String(std::env::var(name).with_context(|| {
            format!("missing environment variable {name}")
        })?));
    }
    let mut path = key.split('.');
    let root = path.next().context("empty variable")?;
    let mut value = variables
        .get(root)
        .with_context(|| format!("missing variable {root}"))?;
    for part in path {
        value = value
            .get(part)
            .with_context(|| format!("missing variable field {key}"))?;
    }
    Ok(value.clone())
}

pub(super) fn text(value: &str, variables: &Variables) -> Result<String> {
    render(&Value::String(value.into()), variables)?
        .as_str()
        .map(str::to_owned)
        .context("expected a string template")
}

pub(super) fn url(base: &str, path: &str, variables: &Variables) -> Result<String> {
    let path = text(path, variables)?;
    super::super::scenario::validate_path(&path)?;
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let parsed = super::super::scenario::validate_url(&url)?;
    ensure!(
        parsed.origin() == super::super::scenario::validate_url(base)?.origin(),
        "request escaped backend origin"
    );
    Ok(url)
}

pub(super) fn validate_environment(template: &str) -> Result<()> {
    for part in template.split("${env:").skip(1) {
        let name = part
            .split_once('}')
            .context("unclosed environment reference")?
            .0;
        ensure!(
            std::env::var(name).is_ok(),
            "missing environment variable {name}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn template_preserves_json_types_and_rejects_missing_values() {
        let variables = BTreeMap::from([
            ("game_index".into(), Value::from(100)),
            ("move".into(), serde_json::json!({"uci":"e2e4"})),
        ]);
        assert_eq!(render(&serde_json::json!({"id":"${game_index}","uci":"${move.uci}","path":"/game/${game_index}"}),&variables).unwrap(),
            serde_json::json!({"id":100,"uci":"e2e4","path":"/game/100"}));
        assert!(render(&Value::String("${missing}".into()), &variables).is_err());
        assert!(url("http://localhost:8090", "//evil.test/", &variables).is_err());
    }
}
