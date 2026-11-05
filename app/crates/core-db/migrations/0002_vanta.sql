-- Vanta additions on top of the blueprint DDL.
ALTER TABLE profiles ADD COLUMN noise_epoch INTEGER NOT NULL DEFAULT 0;
ALTER TABLE profiles ADD COLUMN storage_key_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE profiles ADD COLUMN last_exit_country TEXT;

-- FTS rows are maintained by the application in the same transaction as each write
-- (profiles_fts is a plain FTS5 table keyed by profile_id).
CREATE INDEX IF NOT EXISTS idx_profiles_name ON profiles(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_profiles_pinned ON profiles(pinned DESC, updated_at DESC);
