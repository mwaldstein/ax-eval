use regex::Regex;
use serde_json::Value;

#[derive(Debug)]
pub(super) struct JsonAssertionOutcome {
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum JsonAssertionError {
    Path(String),
    Assertion(String),
}

pub(super) fn evaluate_json_path_assertion(
    json: &Value,
    path: &str,
    assertion: &str,
) -> std::result::Result<JsonAssertionOutcome, JsonAssertionError> {
    let value = resolve_json_path(json, path).map_err(JsonAssertionError::Path)?;
    let (passed, detail) =
        evaluate_json_assertion(value, assertion).map_err(JsonAssertionError::Assertion)?;

    Ok(JsonAssertionOutcome { passed, detail })
}

#[derive(Debug)]
enum JsonPathSegment {
    Key(String),
    Index(usize),
}

fn parse_json_path(path: &str) -> std::result::Result<Vec<JsonPathSegment>, String> {
    if !path.starts_with('$') {
        return Err("path must start with '$'".to_string());
    }

    if path == "$" {
        return Ok(Vec::new());
    }

    let chars: Vec<char> = path.chars().collect();
    let mut i = 1;
    let mut segments = Vec::new();

    while i < chars.len() {
        match chars[i] {
            '.' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '.' && chars[i] != '[' {
                    i += 1;
                }
                if start == i {
                    return Err("empty object key in path".to_string());
                }
                let key: String = chars[start..i].iter().collect();
                segments.push(JsonPathSegment::Key(key));
            }
            '[' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != ']' {
                    i += 1;
                }
                if i >= chars.len() || chars[i] != ']' {
                    return Err("unclosed array index bracket".to_string());
                }
                let index_text: String = chars[start..i].iter().collect();
                let index = index_text
                    .parse::<usize>()
                    .map_err(|_| format!("invalid array index '{}'", index_text))?;
                segments.push(JsonPathSegment::Index(index));
                i += 1;
            }
            _ => return Err(format!("unexpected character '{}' in path", chars[i])),
        }
    }

    Ok(segments)
}

fn resolve_json_path<'a>(
    json: &'a Value,
    path: &str,
) -> std::result::Result<Option<&'a Value>, String> {
    let segments = parse_json_path(path)?;
    let mut current = json;

    for segment in segments {
        match segment {
            JsonPathSegment::Key(key) => {
                let Some(next) = current.get(&key) else {
                    return Ok(None);
                };
                current = next;
            }
            JsonPathSegment::Index(index) => {
                let Some(array) = current.as_array() else {
                    return Ok(None);
                };
                let Some(next) = array.get(index) else {
                    return Ok(None);
                };
                current = next;
            }
        }
    }

    Ok(Some(current))
}

fn evaluate_json_assertion(
    value: Option<&Value>,
    assertion: &str,
) -> std::result::Result<(bool, String), String> {
    let trimmed = assertion.trim();

    if trimmed == "exists" {
        let passed = matches!(value, Some(v) if !v.is_null());
        return Ok((passed, "value exists and is not null".to_string()));
    }

    if let Some(expected_text) = trimmed.strip_prefix("equals ") {
        let Some(actual) = value else {
            return Ok((false, "path not found".to_string()));
        };
        let expected = serde_json::from_str::<Value>(expected_text)
            .unwrap_or_else(|_| Value::String(expected_text.to_string()));
        let passed = actual == &expected;
        return Ok((passed, format!("actual={}, expected={}", actual, expected)));
    }

    if let Some(needle) = trimmed.strip_prefix("contains ") {
        let Some(actual) = value else {
            return Ok((false, "path not found".to_string()));
        };
        let Some(text) = actual.as_str() else {
            return Ok((false, "value is not a string".to_string()));
        };
        let passed = text.contains(needle);
        return Ok((passed, format!("substring='{}'", needle)));
    }

    let len_regex = Regex::new(r"^len\s*(>=|==|>)\s*(\d+)$").expect("valid len regex");
    if let Some(captures) = len_regex.captures(trimmed) {
        let Some(actual) = value else {
            return Ok((false, "path not found".to_string()));
        };
        let operator = captures
            .get(1)
            .map(|m| m.as_str())
            .ok_or_else(|| "missing length operator".to_string())?;
        let expected_len = captures
            .get(2)
            .ok_or_else(|| "missing length value".to_string())?
            .as_str()
            .parse::<usize>()
            .map_err(|_| "length must be a non-negative integer".to_string())?;

        let actual_len = if let Some(array) = actual.as_array() {
            array.len()
        } else if let Some(object) = actual.as_object() {
            object.len()
        } else {
            return Ok((false, "value is not an array or object".to_string()));
        };

        let passed = match operator {
            ">=" => actual_len >= expected_len,
            "==" => actual_len == expected_len,
            ">" => actual_len > expected_len,
            _ => return Err(format!("unsupported length operator '{}'", operator)),
        };

        return Ok((
            passed,
            format!("actual_len={} {} {}", actual_len, operator, expected_len),
        ));
    }

    Err("assertion must be one of: exists, equals <value>, contains <substring>, len >= N, len == N, len > N".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_exists_assertion() {
        let json = serde_json::json!({"meta":{"ok":true}});
        let outcome = evaluate_json_path_assertion(&json, "$.meta.ok", "exists").unwrap();

        assert!(outcome.passed);
    }

    #[test]
    fn evaluates_equals_assertion() {
        let json = serde_json::json!({"count":3});
        let outcome = evaluate_json_path_assertion(&json, "$.count", "equals 3").unwrap();

        assert!(outcome.passed);
    }

    #[test]
    fn evaluates_contains_assertion() {
        let json = serde_json::json!({"msg":"build succeeded"});
        let outcome = evaluate_json_path_assertion(&json, "$.msg", "contains succeeded").unwrap();

        assert!(outcome.passed);
    }

    #[test]
    fn evaluates_len_assertion() {
        let json = serde_json::json!({"items":[1,2,3]});
        let outcome = evaluate_json_path_assertion(&json, "$.items", "len >= 3").unwrap();

        assert!(outcome.passed);
    }

    #[test]
    fn reports_invalid_json_path() {
        let json = serde_json::json!({"ok":true});
        let error = evaluate_json_path_assertion(&json, "ok", "exists").unwrap_err();

        assert_eq!(
            error,
            JsonAssertionError::Path("path must start with '$'".to_string())
        );
    }
}
