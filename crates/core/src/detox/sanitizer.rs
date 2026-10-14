use regex::Regex;
use std::sync::LazyLock;

// Using regex to strip ANSI codes: CSI sequences (colors, cursor moves, clears)
// and OSC sequences (window titles, hyperlinks).
static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1B\[[0-9;?]*[A-Za-z]|\x1B\][^\x07\x1B]*(?:\x07|\x1B\\)").unwrap()
});

// Common secret patterns. Each requires a realistic token body so prose
// mentioning "Bearer tokens" or "sk-" prefixes is not redacted.
static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\bsk-ant-[A-Za-z0-9_\-]{20,}",                                 // Anthropic
        r"|\bsk-(?:proj-)?[A-Za-z0-9_\-]{20,}",                          // OpenAI
        r"|AIza[0-9A-Za-z_\-]{35}",                                      // Google / GCP API key
        r"|gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,}",     // GitHub
        r"|AKIA[0-9A-Z]{16}",                                            // AWS access key ID
        r"|eyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}", // JWT
        r"|(?i:bearer)\s+[A-Za-z0-9\-._~+/]{20,}=*",                     // Bearer token
        r"|-----BEGIN [A-Z ]*PRIVATE KEY-----",                          // PEM private key header
    ))
    .unwrap()
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
        let input = "My key is sk-1234567890abcdef1234567890abcdef1234567890abcdef and Bearer abcDEF1234567890ghijKLMN";
        let (output, count) = sanitize_content(input);
        assert!(output.contains("[REDACTED_SECRET]"));
        assert_eq!(count, 2);
    }

    #[test]
    fn test_sanitize_cloud_keys() {
        let input = concat!(
            "anthropic=sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx ",
            "gcp=AIzaSyA1234567890abcdefghijklmnopqrstuv ",
            "gh=ghp_abcdefghijklmnopqrstuvwxyz0123456789 ",
            "aws=AKIAABCDEFGHIJKLMNOP ",
            "jwt=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
        );
        let (output, count) = sanitize_content(input);
        assert_eq!(count, 5, "output was: {output}");
        assert!(!output.contains("AKIA"));
    }

    #[test]
    fn test_prose_is_not_redacted() {
        let input = "Use Bearer tokens for auth. The bearer token. Prefix keys with sk- and rotate them. Run task-before-startup-sequence-hook, see #ask-multiple-agents-together.";
        let (output, count) = sanitize_content(input);
        assert_eq!(count, 0);
        assert_eq!(output, input);
    }

    #[test]
    fn test_sanitize_extended_ansi() {
        let input = "\x1B[2K\x1B[1;32mok\x1B[0m \x1B]0;title\x07done";
        let (output, _) = sanitize_content(input);
        assert_eq!(output, "ok done");
    }
}

