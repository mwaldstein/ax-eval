use super::super::redact::redact_sensitive;

#[test]
fn test_redact_api_key_sk() {
    let input = "API key: sk-12345678901234567890";
    let output = redact_sensitive(input);
    assert!(output.contains("[REDACTED]"));
    assert!(!output.contains("sk-12345678901234567890"));
}

#[test]
fn test_redact_bearer_token() {
    let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
    let output = redact_sensitive(input);
    assert!(output.contains("Bearer [REDACTED]"));
    assert!(!output.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
}

#[test]
fn test_redact_api_key_header() {
    let input = "api-key: abc123xyz789";
    let output = redact_sensitive(input);
    assert!(output.contains("[REDACTED]"));
    assert!(!output.contains("abc123xyz789"));
}

#[test]
fn test_redact_json_authorization_header() {
    let input = r#"{ "Authorization": "Bearer json-secret-token" }"#;
    let output = redact_sensitive(input);
    assert!(output.contains("Bearer [REDACTED]"));
    assert!(!output.contains("json-secret-token"));
}

#[test]
fn test_redact_json_api_key_header() {
    let input = r#"{ "X-API-Key": "json-api-key-secret" }"#;
    let output = redact_sensitive(input);
    assert!(output.contains("[REDACTED]"));
    assert!(!output.contains("json-api-key-secret"));
}

#[test]
fn test_redact_password() {
    let input = "password: mysecret123";
    let output = redact_sensitive(input);
    assert!(output.contains("[REDACTED]"));
    assert!(!output.contains("mysecret123"));
}

#[test]
fn test_redact_secret() {
    let input = "secret: supersecretvalue";
    let output = redact_sensitive(input);
    assert!(output.contains("[REDACTED]"));
    assert!(!output.contains("supersecretvalue"));
}

#[test]
fn test_redact_email() {
    let input = "Contact: user@example.com for support";
    let output = redact_sensitive(input);
    assert!(output.contains("[REDACTED_EMAIL]"));
    assert!(!output.contains("user@example.com"));
}

#[test]
fn test_redact_unix_home_path() {
    let input = "Config at /home/johndoe/.config/app.conf";
    let output = redact_sensitive(input);
    assert!(output.contains("/home/[REDACTED_USER]/"));
    assert!(!output.contains("/home/johndoe/"));
}

#[test]
fn test_redact_macos_home_path() {
    let input = "File at /Users/alicedoe/Desktop/file.txt";
    let output = redact_sensitive(input);
    assert!(output.contains("/Users/[REDACTED_USER]/"));
    assert!(!output.contains("/Users/alicedoe/"));
}

#[test]
fn test_redact_windows_path() {
    let input = "Path C:\\Users\\bobsmith\\Documents";
    let output = redact_sensitive(input);
    assert!(output.contains("C:\\Users\\[REDACTED_USER]\\"));
    assert!(!output.contains("C:\\Users\\bobsmith\\"));
}

#[test]
fn test_redact_multiple_patterns() {
    let input = "Email: test@example.com, API: sk-abc123xyz78912345678, Path: /home/user/file.txt";
    let output = redact_sensitive(input);
    assert!(output.contains("[REDACTED_EMAIL]"));
    assert!(output.contains("[REDACTED]"));
    assert!(output.contains("/home/[REDACTED_USER]/"));
    assert!(!output.contains("test@example.com"));
    assert!(!output.contains("sk-abc123xyz78912345678"));
    assert!(!output.contains("/home/user/"));
}

#[test]
fn test_redact_empty_string() {
    let input = "";
    let output = redact_sensitive(input);
    assert_eq!(output, "");
}

#[test]
fn test_redact_no_sensitive_data() {
    let input = "This is normal text without sensitive information";
    let output = redact_sensitive(input);
    assert_eq!(output, input);
}
