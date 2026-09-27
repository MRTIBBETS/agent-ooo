use regex::Regex;
use std::sync::LazyLock;

// Using regex to strip ANSI codes.
static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1B\[([0-9]{1,2}(;[0-9]{1,2})?)?[m|K]").unwrap()
});

// A regex to match common secret patterns (OpenAI, Anthropic, Bearer tokens).
static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(sk-[a-zA-Z0-9]{48}|Bearer\s+[A-Za-z0-9\-\._~\+/]+=*)").unwrap()
});

/// Sanitizes a string by stripping ANSI escape codes and masking detected secrets.
pub fn sanitize_content(content: &str) -> (String, usize) {
    let mut redactions = 0;
    
    // First, mask secrets
    let masked = SECRET_RE.replace_all(content, |_: &regex::Captures| {
        redactions += 1;
        "[REDACTED_SECRET]"
    });

    // Second, strip ANSI escape codes
    let cleaned = ANSI_RE.replace_all(&masked, "");

    (cleaned.into_owned(), redactions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_ansi() {
        let input = "Hello \x1B[31mWorld\x1B[0m!";
        let (output, _) = sanitize_content(input);
        assert_eq!(output, "Hello World!");
    }

    #[test]
    fn test_sanitize_secrets() {
        let input = "My key is sk-1234567890abcdef1234567890abcdef1234567890abcdef and Bearer token1234567890";
        let (output, count) = sanitize_content(input);
        assert!(output.contains("[REDACTED_SECRET]"));
        assert_eq!(count, 2);
    }
}
