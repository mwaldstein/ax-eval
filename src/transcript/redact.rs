use regex::Regex;

pub fn redact_sensitive(text: &str) -> String {
    let mut redacted = text.to_string();

    let patterns: Vec<(&str, &str)> = vec![
        (r"(?i)(sk-)[a-zA-Z0-9]{20,}", "$1[REDACTED]"),
        (r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]+=*", "Bearer [REDACTED]"),
        (
            r#"(?i)(Authorization["']?\s*[:=]\s*["']?)Bearer\s+[^"',\s}]+(["']?)"#,
            "${1}Bearer [REDACTED]${2}",
        ),
        (
            r#"(?i)((?:X-)?API-?Key["']?\s*[:=]\s*["']?)[^"',\s}]+(["']?)"#,
            "${1}[REDACTED]${2}",
        ),
        (
            r#"(?i)(api[_-]?key|apikey|token):\s*['"]?[A-Za-z0-9\-._~+/=]+['"]?"#,
            "$1: [REDACTED]",
        ),
        (
            r#"(?i)(password|secret|passphrase):\s*['"]?[^\s'"]+['"]?"#,
            "$1: [REDACTED]",
        ),
        (
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b",
            "[REDACTED_EMAIL]",
        ),
        (r"(?i)/home/[A-Za-z0-9_-]+/", "/home/[REDACTED_USER]/"),
        (r"(?i)/Users/[A-Za-z0-9_-]+/", "/Users/[REDACTED_USER]/"),
        (
            r"(?i)C:\\Users\\[A-Za-z0-9_-]+\\",
            "C:\\Users\\[REDACTED_USER]\\",
        ),
    ];

    for (pattern, replacement) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            redacted = re.replace_all(&redacted, replacement).to_string();
        }
    }

    redacted
}
