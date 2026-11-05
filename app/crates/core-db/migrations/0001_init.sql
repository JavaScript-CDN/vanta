-- Verbatim from blueprint 04 §4.3 (migration 0001_init.sql).
-- ============ meta ============
CREATE TABLE schema_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
INSERT INTO schema_meta(key, value) VALUES ('schema_version', '1');

-- ============ folders (tree) ============
CREATE TABLE folders (
  id          TEXT PRIMARY KEY,                 -- uuid v7 (time-ordered)
  parent_id   TEXT REFERENCES folders(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  color       TEXT,                             -- token name e.g. 'teal' or '#RRGGBB'
  icon        TEXT,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,                 -- unix ms
  updated_at  INTEGER NOT NULL,
  UNIQUE(parent_id, name)
);
CREATE INDEX idx_folders_parent ON folders(parent_id);

-- ============ tags ============
CREATE TABLE tags (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
  color       TEXT NOT NULL DEFAULT 'slate',
  created_at  INTEGER NOT NULL
);

-- ============ fingerprint sets (real-device consistent bundles) ============
-- One row = one coherent device description (OS build, browser major, GPU, fonts, screen, ...)
-- Imported from signed packs; never edited by the user directly (user edits go into profile overrides).
CREATE TABLE fingerprint_sets (
  id               TEXT PRIMARY KEY,            -- content hash (sha256 of canonical JSON)
  pack_id          TEXT NOT NULL,               -- which signed pack it came from
  os_family        TEXT NOT NULL CHECK (os_family IN ('windows','macos','linux','android','ios')),
  os_version       TEXT NOT NULL,
  device_class     TEXT NOT NULL,               -- 'desktop','laptop','phone','tablet'
  chrome_major_min INTEGER NOT NULL,            -- range in which this set is plausible
  chrome_major_max INTEGER NOT NULL,
  gpu_vendor       TEXT NOT NULL,
  gpu_renderer     TEXT NOT NULL,
  screen_w         INTEGER NOT NULL,
  screen_h         INTEGER NOT NULL,
  dpr              REAL    NOT NULL,
  cpu_cores        INTEGER NOT NULL,
  device_memory_gb REAL    NOT NULL,
  weight           REAL    NOT NULL DEFAULT 1.0, -- population weight for sampling
  payload          BLOB    NOT NULL,            -- canonical JSON (zstd) of the full set
  created_at       INTEGER NOT NULL
);
CREATE INDEX idx_fps_os      ON fingerprint_sets(os_family, os_version);
CREATE INDEX idx_fps_chrome  ON fingerprint_sets(chrome_major_min, chrome_major_max);
CREATE INDEX idx_fps_gpu     ON fingerprint_sets(gpu_vendor);

-- ============ proxies ============
CREATE TABLE proxies (
  id              TEXT PRIMARY KEY,
  label           TEXT,
  kind            TEXT NOT NULL CHECK (kind IN ('http','https','socks5','ssh','wireguard','openvpn')),
  host            TEXT,                         -- NULL for wireguard/openvpn (config blob instead)
  port            INTEGER,
  username        TEXT,
  password_enc    BLOB,                         -- encrypted with vault data key (double layer)
  config_enc      BLOB,                         -- wg/ovpn config, encrypted
  rotate_url      TEXT,                         -- optional "change IP" link
  sticky_template TEXT,                         -- e.g. 'user-session-{rand8}' for session generators
  udp_supported   INTEGER,                      -- NULL unknown, 0/1 from last check
  ipv6_supported  INTEGER,
  last_check_at   INTEGER,
  last_status     TEXT CHECK (last_status IN ('ok','slow','fail','auth_fail','unknown')) DEFAULT 'unknown',
  last_latency_ms INTEGER,
  exit_ip         TEXT,
  exit_country    TEXT,                         -- ISO-3166-1 alpha-2
  exit_region     TEXT,
  exit_city       TEXT,
  exit_timezone   TEXT,                         -- IANA
  exit_asn        INTEGER,
  exit_isp        TEXT,
  exit_type       TEXT,                         -- 'residential','mobile','datacenter','isp','unknown'
  tags_csv        TEXT,                         -- lightweight proxy tags
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
);
CREATE INDEX idx_proxies_country ON proxies(exit_country);
CREATE INDEX idx_proxies_status  ON proxies(last_status);

-- ============ templates (for bulk create) ============
CREATE TABLE profile_templates (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  spec        TEXT NOT NULL,                    -- JSON TemplateSpec (see §6)
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

-- ============ profiles ============
CREATE TABLE profiles (
  id                  TEXT PRIMARY KEY,         -- uuid v7
  name                TEXT NOT NULL,
  folder_id           TEXT REFERENCES folders(id) ON DELETE SET NULL,
  color               TEXT,
  notes               TEXT,                     -- markdown, stored encrypted by DB
  status              TEXT NOT NULL DEFAULT 'idle'
                      CHECK (status IN ('idle','queued','starting','running','stopping','crashed','locked')),
  platform            TEXT NOT NULL CHECK (platform IN ('desktop','android','ios')),
  fingerprint_set_id  TEXT NOT NULL REFERENCES fingerprint_sets(id),
  fp_seed             BLOB NOT NULL,            -- 32 random bytes; deterministic per-profile variation
  fp_overrides        TEXT NOT NULL DEFAULT '{}', -- JSON: user overrides on top of the set
  engine_version      TEXT NOT NULL,            -- e.g. '156.0.xxxx.yy' pinned build
  engine_policy       TEXT NOT NULL DEFAULT 'follow_latest'
                      CHECK (engine_policy IN ('follow_latest','pinned')),
  proxy_id            TEXT REFERENCES proxies(id) ON DELETE SET NULL,
  geo_mode            TEXT NOT NULL DEFAULT 'auto_from_proxy'
                      CHECK (geo_mode IN ('auto_from_proxy','manual','host')),
  timezone            TEXT,                     -- resolved value used at last launch
  locale              TEXT,                     -- BCP-47 e.g. 'bn-IN'
  languages           TEXT,                     -- JSON array
  geo_lat             REAL,
  geo_lon             REAL,
  geo_accuracy_m      REAL,
  webrtc_mode         TEXT NOT NULL DEFAULT 'proxied'
                      CHECK (webrtc_mode IN ('proxied','public_only','disabled_present','real')),
  dns_mode            TEXT NOT NULL DEFAULT 'via_proxy'
                      CHECK (dns_mode IN ('via_proxy','doh_matched','system')),
  ipv6_mode           TEXT NOT NULL DEFAULT 'block_if_proxy_v4'
                      CHECK (ipv6_mode IN ('block_if_proxy_v4','allow','block')),
  block_launch_on_mismatch INTEGER NOT NULL DEFAULT 1,
  start_urls          TEXT NOT NULL DEFAULT '[]', -- JSON array
  extensions          TEXT NOT NULL DEFAULT '[]', -- JSON array of extension ids from local library
  ram_limit_mb        INTEGER,                  -- NULL = global default
  cpu_limit_pct       INTEGER,                  -- NULL = global default; 100 = one full core
  launch_priority     INTEGER NOT NULL DEFAULT 0,
  password_protected  INTEGER NOT NULL DEFAULT 0, -- extra per-profile passphrase
  storage_key_wrapped BLOB NOT NULL,            -- per-profile data key, wrapped by vault KEK
  health_score        INTEGER,                  -- last score 0..100
  health_level        TEXT CHECK (health_level IN ('green','yellow','red')),
  health_checked_at   INTEGER,
  last_launched_at    INTEGER,
  launch_count        INTEGER NOT NULL DEFAULT 0,
  pinned              INTEGER NOT NULL DEFAULT 0,
  deleted_at          INTEGER,                  -- soft delete → Trash
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL
);
CREATE INDEX idx_profiles_folder   ON profiles(folder_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_profiles_status   ON profiles(status);
CREATE INDEX idx_profiles_proxy    ON profiles(proxy_id);
CREATE INDEX idx_profiles_health   ON profiles(health_level, health_score);
CREATE INDEX idx_profiles_updated  ON profiles(updated_at DESC);
CREATE INDEX idx_profiles_deleted  ON profiles(deleted_at);
-- uniqueness guard: no two live profiles may resolve to the same fingerprint identity hash
CREATE TABLE profile_fp_identity (
  profile_id     TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
  identity_hash  BLOB NOT NULL UNIQUE           -- sha256 over the "distinguishing tuple" (§10.2)
);

CREATE TABLE profile_tags (
  profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  tag_id     TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (profile_id, tag_id)
);
CREATE INDEX idx_profile_tags_tag ON profile_tags(tag_id);

-- Full-text search over name, notes, tags, proxy label/country (contentless; rebuilt by triggers)
CREATE VIRTUAL TABLE profiles_fts USING fts5(
  profile_id UNINDEXED, name, notes, tags, proxy, tokenize = 'unicode61 remove_diacritics 2'
);
-- (unicode61 tokenizes Bengali script by code points; acceptable for prefix search; see §14.6)

-- ============ saved filters ============
CREATE TABLE saved_filters (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  query       TEXT NOT NULL,                    -- JSON FilterQuery (§9.4)
  pinned      INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL
);

-- ============ workflows (local, visual) ============
CREATE TABLE workflows (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  graph       TEXT NOT NULL,                    -- JSON {nodes, edges, version}
  description TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE TABLE workflow_runs (
  id           TEXT PRIMARY KEY,
  workflow_id  TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
  profile_id   TEXT REFERENCES profiles(id) ON DELETE SET NULL,
  status       TEXT NOT NULL CHECK (status IN ('queued','running','succeeded','failed','cancelled')),
  started_at   INTEGER,
  finished_at  INTEGER,
  error        TEXT,
  step_log     TEXT                             -- JSON array, truncated
);

-- ============ health results ============
CREATE TABLE health_runs (
  id           TEXT PRIMARY KEY,
  profile_id   TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  phase        TEXT NOT NULL CHECK (phase IN ('pre_launch','post_launch','manual')),
  engine_version TEXT NOT NULL,
  score        INTEGER NOT NULL,
  level        TEXT NOT NULL,
  results      BLOB NOT NULL,                   -- zstd JSON of CheckResult[] (file 05)
  created_at   INTEGER NOT NULL
);
CREATE INDEX idx_health_profile ON health_runs(profile_id, created_at DESC);

-- ============ jobs & undo journal ============
CREATE TABLE jobs (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL,                    -- 'bulk_create','bulk_proxy','import_cookies',...
  params      TEXT NOT NULL,
  status      TEXT NOT NULL CHECK (status IN ('queued','running','succeeded','failed','cancelled','partially_succeeded')),
  total       INTEGER NOT NULL DEFAULT 0,
  done        INTEGER NOT NULL DEFAULT 0,
  failed      INTEGER NOT NULL DEFAULT 0,
  undo_until  INTEGER,                          -- unix ms; NULL = not undoable
  created_at  INTEGER NOT NULL,
  finished_at INTEGER
);
CREATE TABLE job_journal (
  job_id      TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  seq         INTEGER NOT NULL,
  entity      TEXT NOT NULL,                    -- 'profile','proxy',...
  entity_id   TEXT NOT NULL,
  before_json TEXT,                             -- NULL for inserts
  after_json  TEXT,                             -- NULL for deletes
  PRIMARY KEY (job_id, seq)
);

-- ============ logs (activity, local only) ============
CREATE TABLE logs (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  ts          INTEGER NOT NULL,
  level       TEXT NOT NULL CHECK (level IN ('debug','info','warn','error')),
  category    TEXT NOT NULL,                    -- 'launch','proxy','health','import','vault','workflow','sync'
  profile_id  TEXT,
  message_key TEXT NOT NULL,                    -- i18n key, never free text with secrets
  params      TEXT                              -- JSON for message interpolation (redacted)
);
CREATE INDEX idx_logs_ts       ON logs(ts DESC);
CREATE INDEX idx_logs_profile  ON logs(profile_id, ts DESC);

-- ============ settings ============
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL                           -- JSON
);

-- ============ extensions library (local files only) ============
CREATE TABLE extensions (
  id           TEXT PRIMARY KEY,                -- Chrome extension id
  name         TEXT NOT NULL,
  version      TEXT NOT NULL,
  crx_sha256   TEXT NOT NULL,                   -- pinned hash; auto-update OFF by default
  source       TEXT NOT NULL,                   -- 'file','store_url'
  added_at     INTEGER NOT NULL
);
