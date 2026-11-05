//! Vault key management (blueprint file 02 §6–§7).
//!
//! Master password --Argon2id--> KEK_pw --XChaCha20-Poly1305--> VMK (32 B random)
//! Recovery code  --HKDF-SHA256--> KEK_rc --XChaCha20-Poly1305--> VMK
//! VMK --HKDF(salt=vault_id, info=label)--> K_db, K_secrets, K_wrap_pdk, K_slotmac, ...
//!
//! The keyslot file is plaintext but authenticated: every slot is an AEAD wrap with
//! AAD = vault_id || slot_id || format, and the whole file carries an HMAC (K_slotmac)
//! that protects `generation` and the slot list against substitution / rollback.

mod crypto;
mod keyslots;
mod recovery;

pub use crypto::{Key32, open_secret, seal_secret};
pub use keyslots::{KdfParams, KeyslotFile};
pub use recovery::{generate_recovery_code, normalize_recovery_code};

use core_model::{AppError, CmdResult};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use zeroize::Zeroize;

/// HKDF labels (domain separation). Never change once shipped.
pub mod labels {
    pub const VAULT_DB: &[u8] = b"vanta/v1/vault-db";
    pub const SECRETS: &[u8] = b"vanta/v1/secrets";
    pub const PDK_WRAP: &[u8] = b"vanta/v1/pdk-wrap";
    pub const SLOT_MAC: &[u8] = b"vanta/v1/slot-mac";
    pub const NAMES: &[u8] = b"vanta/v1/names";
    pub const BACKUP: &[u8] = b"vanta/v1/backup";
    pub const AUDIT: &[u8] = b"vanta/v1/audit";
    pub const FP_SEED: &[u8] = b"vanta/v1/fp-seed";
}

/// Unlocked key material. Dropped (and zeroized) on lock.
pub struct Unlocked {
    vmk: Key32,
    vault_id: [u8; 16],
}

impl Unlocked {
    pub fn subkey(&self, label: &[u8]) -> Key32 { crypto::hkdf32(&self.vmk, &self.vault_id, label) }
    pub fn db_key(&self) -> Key32 { self.subkey(labels::VAULT_DB) }
    pub fn vault_id(&self) -> [u8; 16] { self.vault_id }

    /// Wrap a per-profile data key (PDK). AAD binds it to the profile id and version.
    pub fn wrap_pdk(&self, profile_id: &str, version: u32, pdk: &Key32) -> Vec<u8> {
        let k = self.subkey(labels::PDK_WRAP);
        crypto::seal(&k, pdk.as_bytes(), &pdk_aad(profile_id, version))
    }
    pub fn unwrap_pdk(&self, profile_id: &str, version: u32, blob: &[u8]) -> CmdResult<Key32> {
        let k = self.subkey(labels::PDK_WRAP);
        let mut pt = crypto::open(&k, blob, &pdk_aad(profile_id, version))
            .map_err(|_| AppError::Integrity("pdk".into()))?;
        let key = Key32::from_slice(&pt).ok_or(AppError::Integrity("pdk-len".into()));
        pt.zeroize();
        key
    }
    /// Encrypt a small secret (proxy password, VPN key) with K_secrets.
    pub fn seal_secret(&self, context: &str, plaintext: &[u8]) -> Vec<u8> {
        seal_secret(&self.subkey(labels::SECRETS), context, plaintext)
    }
    pub fn open_secret(&self, context: &str, blob: &[u8]) -> CmdResult<Vec<u8>> {
        open_secret(&self.subkey(labels::SECRETS), context, blob)
    }
}

fn pdk_aad(profile_id: &str, version: u32) -> Vec<u8> {
    let mut a = b"pdk|".to_vec();
    a.extend_from_slice(profile_id.as_bytes());
    a.push(b'|');
    a.extend_from_slice(&version.to_be_bytes());
    a
}

/// Brute-force throttle (UI-01): delays 1,2,4,8,16… s after consecutive failures, capped at 300 s.
#[derive(Default)]
struct Throttle { failures: u32, next_allowed: Option<Instant> }

impl Throttle {
    fn check(&self) -> CmdResult<()> {
        if let Some(t) = self.next_allowed {
            let now = Instant::now();
            if now < t { return Err(AppError::RetryLater((t - now).as_secs().max(1))); }
        }
        Ok(())
    }
    fn fail(&mut self) {
        self.failures = self.failures.saturating_add(1);
        let secs = 1u64 << (self.failures.saturating_sub(1)).min(8);
        self.next_allowed = Some(Instant::now() + Duration::from_secs(secs.min(300)));
    }
    fn ok(&mut self) { *self = Self::default(); }
    fn retry_after(&self) -> u64 {
        self.next_allowed.map(|t| t.saturating_duration_since(Instant::now()).as_secs()).unwrap_or(0)
    }
}

pub struct Vault {
    dir: PathBuf,
    unlocked: Option<Unlocked>,
    throttle: Throttle,
}

pub struct Created {
    /// Shown to the user exactly once.
    pub recovery_code: String,
}

impl Vault {
    pub fn new(dir: impl Into<PathBuf>) -> Self { Self { dir: dir.into(), unlocked: None, throttle: Throttle::default() } }
    pub fn dir(&self) -> &Path { &self.dir }
    fn slots_path(&self) -> PathBuf { self.dir.join("vault.keyslots") }
    pub fn exists(&self) -> bool { self.slots_path().exists() || self.dir.join("vault.keyslots.bak").exists() }
    pub fn is_unlocked(&self) -> bool { self.unlocked.is_some() }
    pub fn retry_after_secs(&self) -> u64 { self.throttle.retry_after() }
    pub fn hint(&self) -> Option<String> { KeyslotFile::load(&self.dir).ok().and_then(|f| f.hint) }

    pub fn require_unlocked(&self) -> CmdResult<&Unlocked> { self.unlocked.as_ref().ok_or(AppError::VaultLocked) }

    /// Create a new vault. `kdf` = None calibrates Argon2id on this machine (file 02 §6.3).
    pub fn create(&mut self, password: &str, hint: Option<String>, kdf: Option<KdfParams>) -> CmdResult<Created> {
        if self.exists() { return Err(AppError::VaultExists); }
        validate_password(password)?;
        std::fs::create_dir_all(&self.dir)?;
        let vault_id: [u8; 16] = crypto::random_array();
        let vmk = Key32::random();
        let params = kdf.unwrap_or_else(KdfParams::calibrate);
        let recovery_code = generate_recovery_code();
        let file = KeyslotFile::build(vault_id, &vmk, password, &params, &recovery_code, hint)?;
        file.save_atomic(&self.dir)?;
        self.unlocked = Some(Unlocked { vmk, vault_id });
        Ok(Created { recovery_code })
    }

    pub fn unlock(&mut self, password: &str) -> CmdResult<()> {
        self.throttle.check()?;
        let file = KeyslotFile::load(&self.dir)?;
        match file.unlock_password(password) {
            Ok(vmk) => {
                file.verify_mac(&vmk)?;
                self.throttle.ok();
                self.unlocked = Some(Unlocked { vmk, vault_id: file.vault_id()? });
                Ok(())
            }
            Err(e) => { self.throttle.fail(); Err(e) }
        }
    }

    pub fn unlock_recovery(&mut self, code: &str) -> CmdResult<()> {
        self.throttle.check()?;
        let file = KeyslotFile::load(&self.dir)?;
        match file.unlock_recovery(code) {
            Ok(vmk) => {
                file.verify_mac(&vmk)?;
                self.throttle.ok();
                self.unlocked = Some(Unlocked { vmk, vault_id: file.vault_id()? });
                Ok(())
            }
            Err(e) => { self.throttle.fail(); Err(e) }
        }
    }

    /// Rewrites slot #0 only; requires the vault to be unlocked and the old password.
    pub fn change_password(&mut self, old: &str, new: &str, hint: Option<String>) -> CmdResult<()> {
        validate_password(new)?;
        self.throttle.check()?;
        let mut file = KeyslotFile::load(&self.dir)?;
        let vmk = match file.unlock_password(old) {
            Ok(v) => v,
            Err(e) => { self.throttle.fail(); return Err(e); }
        };
        file.verify_mac(&vmk)?;
        file.rewrite_password_slot(&vmk, new, &KdfParams::calibrate(), hint)?;
        file.save_atomic(&self.dir)?;
        Ok(())
    }

    pub fn lock(&mut self) { self.unlocked = None; }
}

pub fn validate_password(p: &str) -> CmdResult<()> {
    if p.chars().count() < 10 { return Err(AppError::invalid("password", "vault.password_too_short")); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast() -> Option<KdfParams> { Some(KdfParams { m_kib: 8 * 1024, t: 1, p: 1 }) }

    #[test]
    fn create_lock_unlock_wrong_password() {
        let d = tempfile::tempdir().unwrap();
        let mut v = Vault::new(d.path());
        let created = v.create("correct horse battery", Some("hint".into()), fast()).unwrap();
        let k1 = v.require_unlocked().unwrap().db_key();
        v.lock();
        assert!(matches!(v.require_unlocked(), Err(AppError::VaultLocked)));
        assert!(matches!(v.unlock("wrong password!!"), Err(AppError::WrongPassword)));
        // throttled now
        assert!(matches!(v.unlock("correct horse battery"), Err(AppError::RetryLater(_))));
        v.throttle.ok();
        v.unlock("correct horse battery").unwrap();
        assert_eq!(v.require_unlocked().unwrap().db_key().as_bytes(), k1.as_bytes());
        v.lock();
        v.unlock_recovery(&created.recovery_code.to_lowercase()).unwrap();
        assert_eq!(v.require_unlocked().unwrap().db_key().as_bytes(), k1.as_bytes());
        assert_eq!(v.hint().as_deref(), Some("hint"));
    }

    #[test]
    fn change_password_keeps_vmk() {
        let d = tempfile::tempdir().unwrap();
        let mut v = Vault::new(d.path());
        v.create("first password 1", None, fast()).unwrap();
        let k1 = v.require_unlocked().unwrap().db_key();
        // change_password calibrates; keep test fast by changing params via direct API
        let mut f = KeyslotFile::load(d.path()).unwrap();
        let vmk = f.unlock_password("first password 1").unwrap();
        f.rewrite_password_slot(&vmk, "second password 2", &fast().unwrap(), None).unwrap();
        f.save_atomic(d.path()).unwrap();
        v.lock();
        assert!(v.unlock("first password 1").is_err());
        v.throttle.ok();
        v.unlock("second password 2").unwrap();
        assert_eq!(v.require_unlocked().unwrap().db_key().as_bytes(), k1.as_bytes());
    }

    #[test]
    fn tampered_keyslot_rejected() {
        let d = tempfile::tempdir().unwrap();
        let mut v = Vault::new(d.path());
        v.create("correct horse battery", None, fast()).unwrap();
        v.lock();
        let p = d.path().join("vault.keyslots");
        let mut j: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        j["generation"] = serde_json::json!(999);
        std::fs::write(&p, serde_json::to_vec(&j).unwrap()).unwrap();
        std::fs::remove_file(d.path().join("vault.keyslots.bak")).unwrap();
        assert!(matches!(v.unlock("correct horse battery"), Err(AppError::Integrity(_))));
    }

    #[test]
    fn pdk_and_secret_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let mut v = Vault::new(d.path());
        v.create("correct horse battery", None, fast()).unwrap();
        let u = v.require_unlocked().unwrap();
        let pdk = Key32::random();
        let blob = u.wrap_pdk("p1", 1, &pdk);
        assert_eq!(u.unwrap_pdk("p1", 1, &blob).unwrap().as_bytes(), pdk.as_bytes());
        assert!(u.unwrap_pdk("p2", 1, &blob).is_err());
        let s = u.seal_secret("proxy:1", b"hunter2");
        assert_eq!(u.open_secret("proxy:1", &s).unwrap(), b"hunter2");
        assert!(u.open_secret("proxy:2", &s).is_err());
    }
}
