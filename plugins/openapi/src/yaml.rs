//! YAML, read as YAML 1.2, as OpenAPI recommends, with yaml-rust2, into
//! JSON's values, as serde_json has them, so a document reads the same in
//! either.

use std::str::FromStr;

use serde_json::{Map, Number, Value};
use yaml_rust2::{Yaml, YamlLoader};

/// The first document in `text`, as JSON, or why it can't be read.
pub fn parse(text: &str) -> Result<Value, String> {
    let documents = YamlLoader::load_from_str(text).map_err(|error| error.to_string())?;
    documents.into_iter().next().map_or(Ok(Value::Null), json)
}

/// `yaml` as JSON. An alias is already what it names, as yaml-rust2 reads
/// it.
fn json(yaml: Yaml) -> Result<Value, String> {
    Ok(match yaml {
        Yaml::Real(text) => real(&text),
        Yaml::Integer(integer) => Value::from(integer),
        Yaml::String(text) => Value::String(text),
        Yaml::Boolean(boolean) => Value::Bool(boolean),
        Yaml::Array(items) => Value::Array(items.into_iter().map(json).collect::<Result<_, _>>()?),
        Yaml::Hash(hash) => {
            let mut mapping = Map::new();
            for (key, value) in hash {
                let key = key_text(key)?;
                if mapping.contains_key(&key) {
                    return Err(format!("`{key}` is a key twice in a mapping"));
                }
                mapping.insert(key, json(value)?);
            }
            Value::Object(mapping)
        }
        Yaml::Null | Yaml::Alias(_) | Yaml::BadValue => Value::Null,
    })
}

/// A number YAML reads as real, like `1.5`, or an integer too big for an
/// `i64`, as it's written, if JSON can write it; else its value; else, like
/// `.inf`, its text.
fn real(text: &str) -> Value {
    let unsigned = text.strip_prefix('+').unwrap_or(text);
    let number = (Number::from_str(unsigned).ok())
        .or_else(|| unsigned.parse().ok().and_then(Number::from_f64));
    number.map_or_else(|| Value::String(text.to_string()), Value::Number)
}

/// A mapping's key as JSON's, which is text: a number or `true` as it
/// reads, like `200`, as OpenAPI's responses are keyed.
fn key_text(key: Yaml) -> Result<String, String> {
    match key {
        Yaml::String(text) | Yaml::Real(text) => Ok(text),
        Yaml::Integer(integer) => Ok(integer.to_string()),
        Yaml::Boolean(boolean) => Ok(boolean.to_string()),
        Yaml::Array(_) | Yaml::Hash(_) => {
            Err("a mapping's key is a mapping or a list, which JSON can't have".into())
        }
        Yaml::Null | Yaml::Alias(_) | Yaml::BadValue => Ok("null".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_is_read_as_json() {
        let yaml = "openapi: 3.1.0\nresponses:\n  200: ok\n  default: no\nlist: [1, true, ~]\n";
        let json = parse(yaml).unwrap();
        assert_eq!(
            json.to_string(),
            r#"{"openapi":"3.1.0","responses":{"200":"ok","default":"no"},"list":[1,true,null]}"#
        );
    }

    #[test]
    fn numbers_are_kept_as_theyre_written() {
        let json = parse("a: 1.10\nb: +2.5\nc: .5\nd: 99999999999999999999999\ne: .inf\n");
        assert_eq!(
            json.unwrap().to_string(),
            r#"{"a":1.10,"b":2.5,"c":0.5,"d":99999999999999999999999,"e":".inf"}"#
        );
    }

    #[test]
    fn keys_are_text() {
        let json = parse("1.5: a\ntrue: b\n~: c\n").unwrap();
        assert_eq!(json.to_string(), r#"{"1.5":"a","true":"b","null":"c"}"#);
        let error = parse("? [a]\n: b\n").unwrap_err();
        assert!(error.contains("which JSON can't have"), "{error}");
        let error = parse("200: a\n'200': b\n").unwrap_err();
        assert_eq!(error, "`200` is a key twice in a mapping");
        let error = parse("a: 1\na: 2\n").unwrap_err();
        assert!(error.contains("duplicated key"), "{error}");
    }

    #[test]
    fn aliases_are_what_they_name() {
        let json = parse("x: &pet {name: Rex}\ny: *pet\n").unwrap();
        assert_eq!(json["y"]["name"], "Rex");
    }

    #[test]
    fn an_empty_file_is_null_and_bad_yaml_says_where() {
        assert_eq!(parse(""), Ok(Value::Null));
        assert_eq!(parse("a: 1\n---\nb: 2\n").unwrap()["a"], 1);
        let error = parse("a: [1,\n").unwrap_err();
        assert!(error.contains("line 2"), "{error}");
    }
}
