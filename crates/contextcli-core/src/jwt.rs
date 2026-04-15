//! Best-effort JWT expiry extraction.
//!
//! Tokens stored by adapters may be JWTs (e.g. Vercel, Supabase) or opaque
//! strings (e.g. GitHub PATs).  This module attempts to decode the payload
//! section of a JWT and extract the `exp` claim.  If the token is not a JWT
//! or has no `exp`, it returns `None` — no error.

use std::time::{SystemTime, UNIX_EPOCH};

/// Extract the `exp` claim (Unix timestamp) from a JWT.
/// Returns `None` if the token isn't a valid JWT or has no `exp`.
pub fn extract_jwt_expiry(token: &str) -> Option<i64> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let payload = base64url_decode(parts[1])?;
    let json: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    json.get("exp")?.as_i64()
}

/// Human-readable expiry description from a Unix timestamp.
pub fn format_expiry(expires_at: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let diff = expires_at - now;
    if diff <= 0 {
        let ago = (-diff) as u64;
        if ago < 3600 {
            format!("expired {}m ago", ago / 60)
        } else if ago < 86400 {
            format!("expired {}h ago", ago / 3600)
        } else {
            format!("expired {}d ago", ago / 86400)
        }
    } else {
        let remaining = diff as u64;
        if remaining < 3600 {
            format!("expires in {}m", remaining / 60)
        } else if remaining < 86400 {
            format!("expires in {}h", remaining / 3600)
        } else {
            format!("expires in {}d", remaining / 86400)
        }
    }
}

/// Check if a token is expired based on its `exp` timestamp.
pub fn is_expired(expires_at: i64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    expires_at <= now
}

/// Check if a token expires within `days` days.
pub fn expires_within_days(expires_at: i64, days: u64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let threshold = now + (days * 86400) as i64;
    expires_at <= threshold
}

// ── Base64url decoder ────────────────────────────────────────────────────

/// Decode base64url (RFC 4648 §5, no padding) to bytes.
fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    // Convert base64url alphabet to standard base64
    let mut s: String = input.chars().map(|c| match c {
        '-' => '+',
        '_' => '/',
        other => other,
    }).collect();

    // Add padding
    match s.len() % 4 {
        2 => s.push_str("=="),
        3 => s.push('='),
        0 => {}
        _ => return None,
    }

    b64_decode(s.as_bytes())
}

/// Minimal standard base64 decoder.
fn b64_decode(input: &[u8]) -> Option<Vec<u8>> {
    const INVALID: u8 = 0xFF;

    let table: [u8; 128] = {
        let mut t = [INVALID; 128];
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < 64 {
            t[alphabet[i] as usize] = i as u8;
            i += 1;
        }
        t[b'=' as usize] = 0; // padding maps to 0 for bit-shift purposes
        t
    };

    if input.len() % 4 != 0 {
        return None;
    }

    let mut out = Vec::with_capacity(input.len() * 3 / 4);

    for chunk in input.chunks(4) {
        // Validate characters are ASCII and in the table
        let mut vals = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            if b > 127 {
                return None;
            }
            let v = table[b as usize];
            if v == INVALID && b != b'=' {
                return None;
            }
            vals[i] = v;
        }

        let n = (vals[0] as u32) << 18
            | (vals[1] as u32) << 12
            | (vals[2] as u32) << 6
            | vals[3] as u32;

        out.push((n >> 16) as u8);
        if chunk[2] != b'=' {
            out.push((n >> 8) as u8);
        }
        if chunk[3] != b'=' {
            out.push(n as u8);
        }
    }

    Some(out)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_jwt_expiry() {
        // Craft a minimal JWT: header.payload.signature
        // Header: {"alg":"HS256","typ":"JWT"}
        // Payload: {"sub":"1234567890","exp":1893456000}  (2030-01-01)
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
                     eyJzdWIiOiIxMjM0NTY3ODkwIiwiZXhwIjoxODkzNDU2MDAwfQ.\
                     fake_signature";
        assert_eq!(extract_jwt_expiry(token), Some(1893456000));
    }

    #[test]
    fn test_extract_no_exp() {
        // Payload: {"sub":"1234567890"}
        let token = "eyJhbGciOiJIUzI1NiJ9.\
                     eyJzdWIiOiIxMjM0NTY3ODkwIn0.\
                     sig";
        assert_eq!(extract_jwt_expiry(token), None);
    }

    #[test]
    fn test_opaque_token() {
        assert_eq!(extract_jwt_expiry("ghp_abc123def456"), None);
    }

    #[test]
    fn test_empty_token() {
        assert_eq!(extract_jwt_expiry(""), None);
    }

    #[test]
    fn test_format_expiry_future() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let result = format_expiry(now + 86400 * 30);
        assert!(result.starts_with("expires in 30d") || result.starts_with("expires in 29d"));
    }

    #[test]
    fn test_format_expiry_past() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let result = format_expiry(now - 86400 * 2);
        assert!(result.starts_with("expired 2d ago") || result.starts_with("expired 1d ago"));
    }

    #[test]
    fn test_is_expired() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!(is_expired(now - 100));
        assert!(!is_expired(now + 100));
    }

    #[test]
    fn test_base64url_decode() {
        // "hello" in base64url = "aGVsbG8"
        let decoded = base64url_decode("aGVsbG8").unwrap();
        assert_eq!(decoded, b"hello");
    }
}
