//! Seed derivation (file 01 §12.2). Only derived keys ever leave the core.
use hkdf::Hkdf;
use sha2::Sha256;

#[derive(Clone, Default)]
pub struct Seeds {
    pub canvas: [u8; 32], pub webgl: [u8; 32], pub audio: [u8; 32], pub text: [u8; 32],
    pub rects: [u8; 32], pub media_salt: [u8; 32], pub battery: [u8; 32], pub misc: [u8; 32],
}

impl std::fmt::Debug for Seeds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Seeds(<redacted>)") }
}

impl Seeds {
    /// `profile_secret`: 32 random bytes made at profile creation (stored encrypted).
    /// `set_id` + `noise_epoch` are mixed in, so regenerating the set or "reset canvas identity"
    /// re-keys noise while editing proxy/name never does.
    pub fn derive(profile_secret: &[u8; 32], set_id: &str, noise_epoch: u32) -> Self {
        let mut salt = set_id.as_bytes().to_vec();
        salt.extend_from_slice(&noise_epoch.to_be_bytes());
        let hk = Hkdf::<Sha256>::new(Some(&salt), profile_secret);
        let out = |label: &str| { let mut k = [0u8; 32]; hk.expand(label.as_bytes(), &mut k).unwrap(); k };
        Self {
            canvas: out("vanta/v1/canvas"), webgl: out("vanta/v1/webgl"), audio: out("vanta/v1/audio"),
            text: out("vanta/v1/text"), rects: out("vanta/v1/rects"), media_salt: out("vanta/v1/media"),
            battery: out("vanta/v1/battery"), misc: out("vanta/v1/misc"),
        }
    }
    /// Key used to pick per-profile bucket values when building a set (stable per profile secret).
    pub fn set_builder_key(profile_secret: &[u8; 32]) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(b"vanta/v1/set-builder"), profile_secret);
        let mut k = [0u8; 32];
        hk.expand(b"misc", &mut k).unwrap();
        k
    }
}
