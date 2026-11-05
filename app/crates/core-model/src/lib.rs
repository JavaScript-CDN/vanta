//! Shared types between the Rust core and the UI (file 04 §5–§6).
//! Field names serialise as camelCase to match the TypeScript contract.

use serde::{Deserialize, Serialize};

pub mod error;
pub use error::{AppError, CmdResult, FieldIssue};

pub type ProfileId = String;
pub type FolderId = String;
pub type TagId = String;
pub type ProxyId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus { Idle, Queued, Starting, Running, Stopping, Crashed, Locked }

impl ProfileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle", Self::Queued => "queued", Self::Starting => "starting",
            Self::Running => "running", Self::Stopping => "stopping", Self::Crashed => "crashed",
            Self::Locked => "locked",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "idle" => Self::Idle, "queued" => Self::Queued, "starting" => Self::Starting,
            "running" => Self::Running, "stopping" => Self::Stopping, "crashed" => Self::Crashed,
            "locked" => Self::Locked, _ => return None,
        })
    }
}

macro_rules! str_enum {
    ($name:ident { $($var:ident => $s:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $s)] $var),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$var => $s),+ } }
            pub fn parse(s: &str) -> Option<Self> { match s { $($s => Some(Self::$var),)+ _ => None } }
        }
    };
}

str_enum!(Platform { Desktop => "desktop", Android => "android", Ios => "ios" });
str_enum!(GeoMode { AutoFromProxy => "auto_from_proxy", Manual => "manual", Host => "host" });
str_enum!(WebRtcMode { Proxied => "proxied", PublicOnly => "public_only", DisabledPresent => "disabled_present", Real => "real" });
str_enum!(DnsMode { ViaProxy => "via_proxy", DohMatched => "doh_matched", System => "system" });
str_enum!(Ipv6Mode { BlockIfProxyV4 => "block_if_proxy_v4", Allow => "allow", Block => "block" });
str_enum!(ProxyKind { Http => "http", Https => "https", Socks5 => "socks5", Ssh => "ssh", Wireguard => "wireguard", Openvpn => "openvpn" });
str_enum!(ProxyStatus { Ok => "ok", Slow => "slow", Fail => "fail", AuthFail => "auth_fail", Unknown => "unknown" });
str_enum!(HealthLevel { Green => "green", Yellow => "yellow", Red => "red" });

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
    pub hint: Option<String>,
    /// Seconds the UI must wait before the next unlock attempt (brute-force delay).
    pub retry_after_secs: u64,
    pub autolock_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRef { pub id: TagId, pub name: String, pub color: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRef { pub id: ProxyId, pub label: Option<String>, pub country: Option<String>, pub status: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthRef { pub score: i64, pub level: HealthLevel, pub checked_at: i64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRow {
    pub id: ProfileId,
    pub name: String,
    pub folder_id: Option<FolderId>,
    pub color: Option<String>,
    pub tags: Vec<TagRef>,
    pub status: ProfileStatus,
    pub platform: Platform,
    pub os_family: String,
    pub os_version: String,
    pub browser_major: u32,
    pub proxy: Option<ProxyRef>,
    pub health: Option<HealthRef>,
    pub notes_preview: Option<String>,
    pub last_launched_at: Option<i64>,
    pub pinned: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoPoint { pub lat: f64, pub lon: f64, pub accuracy_m: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDetail {
    #[serde(flatten)]
    pub row: ProfileRow,
    pub notes: Option<String>,
    pub fingerprint_set_id: String,
    pub fp_overrides: serde_json::Value,
    pub resolved: serde_json::Value,
    pub engine_version: String,
    pub engine_policy: String,
    pub geo_mode: GeoMode,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub languages: Vec<String>,
    pub geo: Option<GeoPoint>,
    pub webrtc_mode: WebRtcMode,
    pub dns_mode: DnsMode,
    pub ipv6_mode: Ipv6Mode,
    pub block_launch_on_mismatch: bool,
    pub start_urls: Vec<String>,
    pub extensions: Vec<String>,
    pub ram_limit_mb: Option<u32>,
    pub cpu_limit_pct: Option<u32>,
    pub launch_priority: i32,
    pub password_protected: bool,
    pub created_at: i64,
}

/// What the editor sends to create a profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDraft {
    pub name: String,
    #[serde(default)] pub folder_id: Option<FolderId>,
    #[serde(default)] pub color: Option<String>,
    #[serde(default)] pub notes: Option<String>,
    #[serde(default = "default_platform")] pub platform: Platform,
    /// Fingerprint set to use; None = sample one that fits `os_family`.
    #[serde(default)] pub fingerprint_set_id: Option<String>,
    #[serde(default)] pub os_family: Option<String>,
    #[serde(default)] pub fp_overrides: Option<serde_json::Value>,
    #[serde(default)] pub proxy_id: Option<ProxyId>,
    #[serde(default = "default_geo")] pub geo_mode: GeoMode,
    #[serde(default)] pub timezone: Option<String>,
    #[serde(default)] pub locale: Option<String>,
    #[serde(default)] pub languages: Option<Vec<String>>,
    #[serde(default)] pub geo: Option<GeoPoint>,
    #[serde(default = "default_webrtc")] pub webrtc_mode: WebRtcMode,
    #[serde(default)] pub start_urls: Vec<String>,
    #[serde(default)] pub tag_ids: Vec<TagId>,
    #[serde(default)] pub ram_limit_mb: Option<u32>,
    #[serde(default)] pub cpu_limit_pct: Option<u32>,
}
fn default_platform() -> Platform { Platform::Desktop }
fn default_geo() -> GeoMode { GeoMode::AutoFromProxy }
fn default_webrtc() -> WebRtcMode { WebRtcMode::Proxied }

/// Partial update; every field optional. `Some(None)` clears a nullable field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePatch {
    pub name: Option<String>,
    #[serde(default, with = "double_option")] pub folder_id: Option<Option<FolderId>>,
    #[serde(default, with = "double_option")] pub color: Option<Option<String>>,
    #[serde(default, with = "double_option")] pub notes: Option<Option<String>>,
    #[serde(default, with = "double_option")] pub proxy_id: Option<Option<ProxyId>>,
    pub fp_overrides: Option<serde_json::Value>,
    pub geo_mode: Option<GeoMode>,
    #[serde(default, with = "double_option")] pub timezone: Option<Option<String>>,
    #[serde(default, with = "double_option")] pub locale: Option<Option<String>>,
    pub languages: Option<Vec<String>>,
    pub webrtc_mode: Option<WebRtcMode>,
    pub dns_mode: Option<DnsMode>,
    pub ipv6_mode: Option<Ipv6Mode>,
    pub block_launch_on_mismatch: Option<bool>,
    pub start_urls: Option<Vec<String>>,
    pub tag_ids: Option<Vec<TagId>>,
    pub pinned: Option<bool>,
    #[serde(default, with = "double_option")] pub ram_limit_mb: Option<Option<u32>>,
    #[serde(default, with = "double_option")] pub cpu_limit_pct: Option<Option<u32>>,
    pub launch_priority: Option<i32>,
}

mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<T: Serialize, S: Serializer>(v: &Option<Option<T>>, s: S) -> Result<S::Ok, S::Error> {
        match v { Some(inner) => inner.serialize(s), None => s.serialize_none() }
    }
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Ok(Some(Option::deserialize(d)?))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortField { Name, UpdatedAt, CreatedAt, LastLaunchedAt, HealthScore, Status }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDir { Asc, Desc }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sort { pub field: SortField, pub dir: SortDir }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterQuery {
    pub text: Option<String>,
    pub folder_ids: Option<Vec<FolderId>>,
    pub include_subfolders: Option<bool>,
    pub tag_ids_all: Option<Vec<TagId>>,
    pub tag_ids_any: Option<Vec<TagId>>,
    pub tag_ids_none: Option<Vec<TagId>>,
    pub statuses: Option<Vec<ProfileStatus>>,
    pub platforms: Option<Vec<Platform>>,
    pub os_families: Option<Vec<String>>,
    pub health_levels: Option<Vec<HealthLevel>>,
    pub health_score_min: Option<i64>,
    pub health_score_max: Option<i64>,
    pub proxy_countries: Option<Vec<String>>,
    pub has_proxy: Option<bool>,
    pub colors: Option<Vec<String>>,
    pub pinned: Option<bool>,
    pub in_trash: Option<bool>,
    pub sort: Option<Sort>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PageReq { pub offset: u64, pub limit: u64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> { pub items: Vec<T>, pub total: u64, pub offset: u64, pub limit: u64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: FolderId, pub parent_id: Option<FolderId>, pub name: String,
    pub color: Option<String>, pub sort_order: i64, pub profile_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagWithCount { pub id: TagId, pub name: String, pub color: String, pub count: u64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub id: ProxyId,
    pub label: Option<String>,
    pub kind: ProxyKind,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    /// Never sent to the UI; true when a password is stored.
    pub has_password: bool,
    pub rotate_url: Option<String>,
    pub last_status: ProxyStatus,
    pub last_latency_ms: Option<i64>,
    pub last_check_at: Option<i64>,
    pub exit_ip: Option<String>,
    pub exit_country: Option<String>,
    pub exit_city: Option<String>,
    pub exit_timezone: Option<String>,
    pub exit_asn: Option<i64>,
    pub exit_isp: Option<String>,
    pub profile_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyDraft {
    #[serde(default)] pub id: Option<ProxyId>,
    #[serde(default)] pub label: Option<String>,
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    #[serde(default)] pub username: Option<String>,
    /// None = keep existing password on update.
    #[serde(default)] pub password: Option<String>,
    #[serde(default)] pub rotate_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCheckResult {
    pub status: ProxyStatus,
    pub latency_ms: Option<i64>,
    pub exit_ip: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
    pub asn: Option<i64>,
    pub isp: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRow {
    pub id: i64, pub ts: i64, pub level: String, pub category: String,
    pub profile_id: Option<String>, pub message_key: String, pub params: serde_json::Value,
}
