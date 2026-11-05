use crate::crypto::random_array;

/// Crockford base32 (no I, L, O, U) — easy to read aloud and type.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// 260-bit recovery code: 52 chars in 13 groups of 4, e.g. `7K2Q-...`.
pub fn generate_recovery_code() -> String {
    let bytes: [u8; 33] = random_array();
    let mut bits = 0u32;
    let mut acc = 0u32;
    let mut chars = Vec::with_capacity(52);
    for b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 && chars.len() < 52 {
            bits -= 5;
            chars.push(ALPHABET[((acc >> bits) & 31) as usize]);
        }
    }
    chars.chunks(4).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("-")
}

/// Upper-case, strip separators/whitespace, map ambiguous characters (Crockford rules).
pub fn normalize_recovery_code(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| match c.to_ascii_uppercase() { 'O' => '0', 'I' | 'L' => '1', o => o })
        .collect()
}

#[cfg(test)]
mod t {
    #[test]
    fn shape() {
        let c = super::generate_recovery_code();
        assert_eq!(c.len(), 52 + 12);
        assert_eq!(super::normalize_recovery_code(&c.to_lowercase()).len(), 52);
    }
}
