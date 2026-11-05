use crate::crypto::{self, Key32, random_array};
use crate::recovery::normalize_recovery_code;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use core_model::{AppError, CmdResult};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use zeroize::Zeroize;

const MAGIC: &str = "VANTAVKS";
const FORMAT: u8 = 1;
const SLOT_PASSWORD: u8 = 0;
const SLOT_RECOVERY: u8 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct KdfParams { pub m_kib: u32, pub t: u32, pub p: u32 }

impl KdfParams {
    pub const DEFAULT: Self = Self { m_kib: 256 * 1024, t: 3, p: 4 };
    pub const FLOOR: Self = Self { m_kib: 64 * 1024, t: 3, p: 4 };

    /// file 02 §6.3: start at m=256 MiB,t=3,p=4; if one derivation > 1.5 s lower m in 32 MiB
    /// steps to 64 MiB; if < 0.4 s raise t up to 6.
    pub fn calibrate() -> Self {
        let mut p = Self::DEFAULT;
        let salt = [7u8; 32];
        loop {
            let t0 = Instant::now();
            let _ = derive_kek("calibration-password", &salt, &p);
            let el = t0.elapsed().as_secs_f64();
            if el > 1.5 && p.m_kib > Self::FLOOR.m_kib {
                p.m_kib = (p.m_kib - 32 * 1024).max(Self::FLOOR.m_kib);
                continue;
            }
            if el < 0.4 && p.t < 6 { p.t += 1; continue; }
            return p;
        }
    }
}

fn derive_kek(password: &str, salt: &[u8], p: &KdfParams) -> CmdResult<Key32> {
    let params = argon2::Params::new(p.m_kib, p.t, p.p, Some(32)).map_err(|_| AppError::Internal)?;
    let a2 = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut out = [0u8; 32];
    a2.hash_password_into(password.as_bytes(), salt, &mut out).map_err(|_| AppError::Internal)?;
    let k = Key32::from_array(out);
    out.zeroize();
    Ok(k)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PasswordKdf { alg: String, v: u32, m_kib: u32, t: u32, p: u32, salt: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Slot {
    id: u8,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")] kdf: Option<PasswordKdf>,
    #[serde(skip_serializing_if = "Option::is_none")] salt: Option<String>,
    wrap: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyslotFile {
    magic: String,
    format: u8,
    vault_id: String,
    generation: u64,
    pub hint: Option<String>,
    slots: Vec<Slot>,
    #[serde(default)] mac: String,
}

fn slot_aad(vault_id: &[u8], slot: u8) -> Vec<u8> {
    let mut a = vault_id.to_vec();
    a.push(slot);
    a.push(FORMAT);
    a
}

fn b64d(s: &str) -> CmdResult<Vec<u8>> { B64.decode(s).map_err(|_| AppError::Integrity("keyslots".into())) }

impl KeyslotFile {
    pub fn build(vault_id: [u8; 16], vmk: &Key32, password: &str, params: &KdfParams, recovery: &str, hint: Option<String>) -> CmdResult<Self> {
        let mut f = Self {
            magic: MAGIC.into(), format: FORMAT, vault_id: B64.encode(vault_id), generation: 1,
            hint, slots: vec![], mac: String::new(),
        };
        f.slots.push(Self::password_slot(&vault_id, vmk, password, params)?);
        let rsalt: [u8; 32] = random_array();
        let kek = Self::recovery_kek(recovery, &rsalt);
        f.slots.push(Slot {
            id: SLOT_RECOVERY, kind: "recovery".into(), kdf: None, salt: Some(B64.encode(rsalt)),
            wrap: B64.encode(crypto::seal(&kek, vmk.as_bytes(), &slot_aad(&vault_id, SLOT_RECOVERY))),
        });
        f.mac = f.compute_mac(vmk)?;
        Ok(f)
    }

    fn password_slot(vault_id: &[u8], vmk: &Key32, password: &str, params: &KdfParams) -> CmdResult<Slot> {
        let salt: [u8; 32] = random_array();
        let kek = derive_kek(password, &salt, params)?;
        Ok(Slot {
            id: SLOT_PASSWORD, kind: "password".into(),
            kdf: Some(PasswordKdf { alg: "argon2id".into(), v: 19, m_kib: params.m_kib, t: params.t, p: params.p, salt: B64.encode(salt) }),
            salt: None,
            wrap: B64.encode(crypto::seal(&kek, vmk.as_bytes(), &slot_aad(vault_id, SLOT_PASSWORD))),
        })
    }

    fn recovery_kek(code: &str, salt: &[u8]) -> Key32 {
        let norm = normalize_recovery_code(code);
        let ikm = crypto::hkdf32(&Key32::from_array(sha256(norm.as_bytes())), salt, b"vanta/v1/rc-kek");
        ikm
    }

    pub fn vault_id(&self) -> CmdResult<[u8; 16]> {
        let v = b64d(&self.vault_id)?;
        v.try_into().map_err(|_| AppError::Integrity("vault_id".into()))
    }

    fn slot(&self, id: u8) -> CmdResult<&Slot> {
        self.slots.iter().find(|s| s.id == id).ok_or(AppError::Integrity("slot missing".into()))
    }

    pub fn unlock_password(&self, password: &str) -> CmdResult<Key32> {
        let s = self.slot(SLOT_PASSWORD)?;
        let k = s.kdf.as_ref().ok_or(AppError::Integrity("kdf".into()))?;
        if k.alg != "argon2id" || k.v != 19 { return Err(AppError::Integrity("kdf alg".into())); }
        // Refuse absurd parameters from a tampered file (DoS guard).
        if k.m_kib > 4 * 1024 * 1024 || k.t > 64 || k.p == 0 || k.p > 64 { return Err(AppError::Integrity("kdf params".into())); }
        let kek = derive_kek(password, &b64d(&k.salt)?, &KdfParams { m_kib: k.m_kib, t: k.t, p: k.p })?;
        let vid = self.vault_id()?;
        let mut pt = crypto::open(&kek, &b64d(&s.wrap)?, &slot_aad(&vid, SLOT_PASSWORD)).map_err(|_| AppError::WrongPassword)?;
        let vmk = Key32::from_slice(&pt).ok_or(AppError::Integrity("vmk".into()));
        pt.zeroize();
        vmk
    }

    pub fn unlock_recovery(&self, code: &str) -> CmdResult<Key32> {
        let s = self.slot(SLOT_RECOVERY)?;
        let salt = b64d(s.salt.as_deref().ok_or(AppError::Integrity("salt".into()))?)?;
        let kek = Self::recovery_kek(code, &salt);
        let vid = self.vault_id()?;
        let mut pt = crypto::open(&kek, &b64d(&s.wrap)?, &slot_aad(&vid, SLOT_RECOVERY)).map_err(|_| AppError::WrongPassword)?;
        let vmk = Key32::from_slice(&pt).ok_or(AppError::Integrity("vmk".into()));
        pt.zeroize();
        vmk
    }

    pub fn rewrite_password_slot(&mut self, vmk: &Key32, new_password: &str, params: &KdfParams, hint: Option<String>) -> CmdResult<()> {
        let vid = self.vault_id()?;
        let slot = Self::password_slot(&vid, vmk, new_password, params)?;
        self.slots.retain(|s| s.id != SLOT_PASSWORD);
        self.slots.insert(0, slot);
        self.hint = hint;
        self.generation += 1;
        self.mac = self.compute_mac(vmk)?;
        Ok(())
    }

    fn compute_mac(&self, vmk: &Key32) -> CmdResult<String> {
        let vid = self.vault_id()?;
        let k = crypto::hkdf32(vmk, &vid, crate::labels::SLOT_MAC);
        let mut body = self.clone();
        body.mac = String::new();
        let bytes = serde_json::to_vec(&body).map_err(|_| AppError::Internal)?;
        let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(k.as_bytes()).map_err(|_| AppError::Internal)?;
        m.update(&bytes);
        Ok(B64.encode(m.finalize().into_bytes()))
    }

    pub fn verify_mac(&self, vmk: &Key32) -> CmdResult<()> {
        use subtle::ConstantTimeEq;
        let expect = self.compute_mac(vmk)?;
        if expect.as_bytes().ct_eq(self.mac.as_bytes()).into() { Ok(()) } else { Err(AppError::Integrity("keyslots mac".into())) }
    }

    fn parse(bytes: &[u8]) -> CmdResult<Self> {
        let f: Self = serde_json::from_slice(bytes).map_err(|_| AppError::Integrity("keyslots parse".into()))?;
        if f.magic != MAGIC || f.format != FORMAT { return Err(AppError::Integrity("keyslots magic".into())); }
        Ok(f)
    }

    /// Load primary; fall back to `.bak` if the primary is missing or unparsable.
    pub fn load(dir: &Path) -> CmdResult<Self> {
        let primary = dir.join("vault.keyslots");
        let backup = dir.join("vault.keyslots.bak");
        match std::fs::read(&primary).map_err(AppError::from).and_then(|b| Self::parse(&b)) {
            Ok(f) => Ok(f),
            Err(_) if backup.exists() => Self::parse(&std::fs::read(&backup)?),
            Err(_) if !primary.exists() => Err(AppError::NoVault),
            Err(e) => Err(e),
        }
    }

    /// tmp + fsync + rename, then the same for `.bak` (file 02 §7.2).
    pub fn save_atomic(&self, dir: &Path) -> CmdResult<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| AppError::Internal)?;
        for name in ["vault.keyslots", "vault.keyslots.bak"] {
            let tmp = dir.join(format!("{name}.tmp"));
            {
                let mut f = std::fs::File::create(&tmp)?;
                f.write_all(&bytes)?;
                f.sync_all()?;
            }
            std::fs::rename(&tmp, dir.join(name))?;
        }
        #[cfg(unix)]
        if let Ok(d) = std::fs::File::open(dir) { let _ = d.sync_all(); }
        Ok(())
    }
}

fn sha256(b: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut h = Sha256::new();
    h.update(b);
    h.finalize().into()
}
