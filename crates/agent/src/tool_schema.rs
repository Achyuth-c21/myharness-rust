//! Tool-argument validation — roadmap Phase 5 (`docs/12` §5.2).
//!
//! Tool schemas are untrusted input (`docs/12` §5.1). The agent validates the
//! model's arguments against the tool's own schema **before** any executor is
//! contacted; an invalid object becomes a model-visible tool error, never an
//! execution (`docs/12` §5.2: "on invalid JSON, surface an error and do not
//! execute").
//!
//! This is a deliberately bounded JSON-Schema subset, not a full validator.
//! Supported keywords: `type`, `enum`, `const`, `required`, `properties`,
//! `additionalProperties` (boolean or schema), `items`, `minItems`,
//! `maxItems`, `minLength`, `maxLength`, `minimum`, `maximum`,
//! `exclusiveMinimum`, `exclusiveMaximum`, `allOf`, `anyOf`, `oneOf`, `not`,
//! and local `$ref` (`#/$defs/...`, `#/definitions/...`). Unsupported
//! constructs (`pattern`, `format`, external `$ref`, conditional schemas) are
//! not enforced. That is a recorded limitation (`docs/14` §9), not a claim of
//! full draft support; the shipped validator's draft/coverage was not
//! captured.
//!
//! Errors never echo argument values: a schema error can contain a secret if
//! the value is interpolated, so only the JSON path and the expectation are
//! returned.

use serde_json::Value;

/// Maximum schema nesting this validator walks.
pub const MAX_SCHEMA_DEPTH: usize = 64;

/// Validate a tool's argument object.
///
/// MCP tools take a JSON object; a `null`/scalar/array argument set is
/// rejected even when the schema does not say `"type": "object"`.
pub fn validate_arguments(schema: &Value, arguments: &Value) -> Result<(), String> {
    if !arguments.is_object() {
        return Err("the tool arguments must be a JSON object".to_string());
    }
    validate_value(schema, schema, arguments, "arguments", 0)
}

fn validate_value(
    root: &Value,
    schema: &Value,
    value: &Value,
    path: &str,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(format!("{path}: the schema is nested too deeply"));
    }
    match schema {
        Value::Bool(true) => Ok(()),
        Value::Bool(false) => Err(format!("{path}: the schema forbids every value")),
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                let resolved = resolve_local_ref(root, reference)
                    .ok_or_else(|| format!("{path}: the schema reference is not local"))?;
                return validate_value(root, resolved, value, path, depth + 1);
            }
            if let Some(types) = object.get("type") {
                if !matches_type(types, value) {
                    return Err(format!(
                        "{path}: expected {}, got {}",
                        describe_types(types),
                        value_kind(value)
                    ));
                }
            }
            if let Some(constant) = object.get("const") {
                if value != constant {
                    return Err(format!("{path}: the value is not the required constant"));
                }
            }
            if let Some(values) = object.get("enum").and_then(Value::as_array) {
                if !values.iter().any(|candidate| candidate == value) {
                    return Err(format!(
                        "{path}: the value is not one of the allowed values"
                    ));
                }
            }
            if let Some(all) = object.get("allOf").and_then(Value::as_array) {
                for (index, inner) in all.iter().enumerate() {
                    validate_value(
                        root,
                        inner,
                        value,
                        &format!("{path}.allOf[{index}]"),
                        depth + 1,
                    )?;
                }
            }
            if let Some(any) = object.get("anyOf").and_then(Value::as_array) {
                let matched = any.iter().enumerate().any(|(index, inner)| {
                    validate_value(
                        root,
                        inner,
                        value,
                        &format!("{path}.anyOf[{index}]"),
                        depth + 1,
                    )
                    .is_ok()
                });
                if !matched {
                    return Err(format!("{path}: no anyOf branch matched"));
                }
            }
            if let Some(one) = object.get("oneOf").and_then(Value::as_array) {
                let matches = one
                    .iter()
                    .enumerate()
                    .filter(|(index, inner)| {
                        validate_value(
                            root,
                            inner,
                            value,
                            &format!("{path}.oneOf[{index}]"),
                            depth + 1,
                        )
                        .is_ok()
                    })
                    .count();
                if matches != 1 {
                    return Err(format!("{path}: exactly one oneOf branch must match"));
                }
            }
            if let Some(inner) = object.get("not") {
                if validate_value(root, inner, value, &format!("{path}.not"), depth + 1).is_ok() {
                    return Err(format!("{path}: the value matches a forbidden schema"));
                }
            }
            match value {
                Value::Object(map) => validate_object(root, object, map, path, depth),
                Value::Array(items) => validate_array(root, object, items, path, depth),
                Value::String(text) => validate_string(object, text, path),
                Value::Number(number) => validate_number(object, number, path),
                Value::Null | Value::Bool(_) => Ok(()),
            }
        }
        _ => Err(format!("{path}: the schema is not an object")),
    }
}

fn validate_object(
    root: &Value,
    schema: &serde_json::Map<String, Value>,
    map: &serde_json::Map<String, Value>,
    path: &str,
    depth: usize,
) -> Result<(), String> {
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for key in required.iter().filter_map(Value::as_str) {
            if !map.contains_key(key) {
                return Err(format!("{path}: missing required property {key:?}"));
            }
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (key, value) in map {
            if let Some(inner) = properties.get(key) {
                validate_value(root, inner, value, &format!("{path}.{key}"), depth + 1)?;
            }
        }
    }
    let additional = schema.get("additionalProperties");
    for key in map.keys() {
        if properties.is_some_and(|properties| properties.contains_key(key)) {
            continue;
        }
        match additional {
            Some(Value::Bool(false)) => {
                return Err(format!("{path}: unexpected property {key:?}"));
            }
            Some(Value::Bool(true)) | None => {}
            Some(inner) => {
                validate_value(
                    root,
                    inner,
                    map.get(key).expect("key came from the map"),
                    &format!("{path}.{key}"),
                    depth + 1,
                )?;
            }
        }
    }
    if let Some(min) = schema.get("minProperties").and_then(Value::as_u64) {
        if (map.len() as u64) < min {
            return Err(format!("{path}: fewer than {min} properties"));
        }
    }
    if let Some(max) = schema.get("maxProperties").and_then(Value::as_u64) {
        if (map.len() as u64) > max {
            return Err(format!("{path}: more than {max} properties"));
        }
    }
    Ok(())
}

fn validate_array(
    root: &Value,
    schema: &serde_json::Map<String, Value>,
    items: &[Value],
    path: &str,
    depth: usize,
) -> Result<(), String> {
    if let Some(min) = schema.get("minItems").and_then(Value::as_u64) {
        if (items.len() as u64) < min {
            return Err(format!("{path}: fewer than {min} items"));
        }
    }
    if let Some(max) = schema.get("maxItems").and_then(Value::as_u64) {
        if (items.len() as u64) > max {
            return Err(format!("{path}: more than {max} items"));
        }
    }
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate_value(
                root,
                item_schema,
                item,
                &format!("{path}[{index}]"),
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn validate_string(
    schema: &serde_json::Map<String, Value>,
    text: &str,
    path: &str,
) -> Result<(), String> {
    let length = text.chars().count() as u64;
    if let Some(min) = schema.get("minLength").and_then(Value::as_u64) {
        if length < min {
            return Err(format!("{path}: shorter than {min} characters"));
        }
    }
    if let Some(max) = schema.get("maxLength").and_then(Value::as_u64) {
        if length > max {
            return Err(format!("{path}: longer than {max} characters"));
        }
    }
    Ok(())
}

fn validate_number(
    schema: &serde_json::Map<String, Value>,
    number: &serde_json::Number,
    path: &str,
) -> Result<(), String> {
    let value = number
        .as_f64()
        .ok_or_else(|| format!("{path}: the number is not finite"))?;
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if value < minimum {
            return Err(format!("{path}: less than the minimum"));
        }
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64) {
        if value > maximum {
            return Err(format!("{path}: greater than the maximum"));
        }
    }
    if let Some(minimum) = schema.get("exclusiveMinimum").and_then(Value::as_f64) {
        if value <= minimum {
            return Err(format!("{path}: not greater than the exclusive minimum"));
        }
    }
    if let Some(maximum) = schema.get("exclusiveMaximum").and_then(Value::as_f64) {
        if value >= maximum {
            return Err(format!("{path}: not less than the exclusive maximum"));
        }
    }
    Ok(())
}

fn resolve_local_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    root.pointer(pointer)
}

fn matches_type(types: &Value, value: &Value) -> bool {
    match types {
        Value::String(kind) => matches_single_type(kind, value),
        Value::Array(kinds) => kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|kind| matches_single_type(kind, value)),
        _ => false,
    }
}

fn matches_single_type(kind: &str, value: &Value) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn describe_types(types: &Value) -> String {
    match types {
        Value::String(kind) => kind.clone(),
        Value::Array(kinds) => kinds
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" or "),
        _ => "a valid type".to_string(),
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_object_with_required_fields_passes() {
        let schema = json!({
            "type": "object",
            "required": ["repo"],
            "properties": {"repo": {"type": "string"}},
            "additionalProperties": false
        });
        assert!(validate_arguments(&schema, &json!({"repo": "prod"})).is_ok());
    }

    #[test]
    fn a_missing_required_field_is_refused_without_echoing_values() {
        let schema = json!({
            "type": "object",
            "required": ["repo"],
            "properties": {"repo": {"type": "string"}}
        });
        let error = validate_arguments(&schema, &json!({"title": "secret-title"})).unwrap_err();
        assert!(error.contains("repo"));
        assert!(!error.contains("secret-title"));
    }

    #[test]
    fn a_wrong_type_is_refused() {
        let schema = json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}}
        });
        assert!(validate_arguments(&schema, &json!({"count": "three"})).is_err());
        assert!(validate_arguments(&schema, &json!({"count": 3})).is_ok());
    }

    #[test]
    fn additional_properties_false_is_enforced() {
        let schema = json!({
            "type": "object",
            "properties": {"repo": {"type": "string"}},
            "additionalProperties": false
        });
        assert!(validate_arguments(&schema, &json!({"repo": "prod", "extra": true})).is_err());
    }

    #[test]
    fn enum_and_one_of_are_enforced() {
        let schema = json!({"oneOf": [{"enum": ["a"]}, {"enum": ["b"]}]});
        assert!(validate_arguments(&schema, &json!({"x": "a"})).is_err());
        let object = json!({
            "type": "object",
            "properties": {"kind": {"enum": ["a", "b"]}}
        });
        assert!(validate_arguments(&object, &json!({"kind": "c"})).is_err());
        assert!(validate_arguments(&object, &json!({"kind": "a"})).is_ok());
    }

    #[test]
    fn local_refs_resolve() {
        let schema = json!({
            "type": "object",
            "properties": {"repo": {"$ref": "#/$defs/repo"}},
            "$defs": {"repo": {"type": "string", "minLength": 2}}
        });
        assert!(validate_arguments(&schema, &json!({"repo": "p"})).is_err());
        assert!(validate_arguments(&schema, &json!({"repo": "prod"})).is_ok());
    }

    #[test]
    fn non_object_arguments_are_refused() {
        let schema = json!({"type": "object"});
        assert!(validate_arguments(&schema, &Value::Null).is_err());
        assert!(validate_arguments(&schema, &json!([])).is_err());
    }
}
