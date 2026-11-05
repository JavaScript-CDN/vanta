use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use core_model::{AppError, CmdResult};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 32-byte secret key, zeroized on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Key32([u8; 32]);

impl Key32 {
    pub fn random() -> Self { Self(random_array()) }
    pub fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() != 32 { return None; }
        let mut k = [0u8; 32];
        k.copy_from_slice(s);
        Some(Self(k))
    }
    pub fn from_array(a: [u8; 32]) -> Self { Self(a) }
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

impl std::fmt::Debug for Key32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Key32(<redacted>)") }
}

pub fn random_array<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("OS RNG unavailable");
    b
}

pub fn hkdf32(ikm: &Key32, salt: &[u8], info: &[u8]) -> Key32 {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm.as_bytes());
    let mut out = [0u8; 32];
    hk.expand(info, &mut out).expect("32 bytes is a valid HKDF-SHA256 length");
    Key32(out)
}

/// XChaCha20-Poly1305 with a fresh 192-bit random nonce. Output = nonce(24) || ct || tag(16).
pub fn seal(key: &Key32, plaintext: &[u8], aad: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let nonce_bytes: [u8; 24] = random_array();
    let nonce = XNonce::from(nonce_bytes);
    let ct = cipher.encrypt(&nonce, Payload { msg: plaintext, aad }).expect("encryption cannot fail");
    let mut out = Vec::with_capacity(24 + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    out
}

pub fn open(key: &Key32, blob: &[u8], aad: &[u8]) -> Result<Vec<u8>, ()> {
    if blob.len() < 24 + 16 { return Err(()); }
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let mut n = [0u8; 24];
    n.copy_from_slice(&blob[..24]);
    let nonce = XNonce::from(n);
    cipher.decrypt(&nonce, Payload { msg: &blob[24..], aad }).map_err(|_| ())
}

pub fn seal_secret(k: &Key32, context: &str, plaintext: &[u8]) -> Vec<u8> {
    seal(k, plaintext, format!("secret|{context}").as_bytes())
}
pub fn open_secret(k: &Key32, context: &str, blob: &[u8]) -> CmdResult<Vec<u8>> {
    open(k, blob, format!("secret|{context}").as_bytes()).map_err(|_| AppError::Integrity("secret".into()))
}
