// Typed wrappers over Tauri commands (contract: blueprint file 04 §5–§6).
import { invoke } from '@tauri-apps/api/core';

export type ProfileStatus = 'idle' | 'queued' | 'starting' | 'running' | 'stopping' | 'crashed' | 'locked';
export type GeoMode = 'auto_from_proxy' | 'manual' | 'host';
export type WebRtcMode = 'proxied' | 'public_only' | 'disabled_present' | 'real';
export type ProxyKind = 'http' | 'https' | 'socks5' | 'ssh' | 'wireguard' | 'openvpn';
export type ProxyStatus = 'ok' | 'slow' | 'fail' | 'auth_fail' | 'unknown';

export interface TagRef { id: string; name: string; color: string }
export interface ProfileRow {
  id: string; name: string; folderId: string | null; color: string | null; tags: TagRef[];
  status: ProfileStatus; platform: string; osFamily: string; osVersion: string; browserMajor: number;
  proxy: { id: string; label: string | null; country: string | null; status: string } | null;
  health: { score: number; level: 'green' | 'yellow' | 'red'; checkedAt: number } | null;
  notesPreview: string | null; lastLaunchedAt: number | null; pinned: boolean; updatedAt: number;
}
export interface ProfileDetail extends ProfileRow {
  notes: string | null; fingerprintSetId: string; fpOverrides: Record<string, unknown>; resolved: any;
  engineVersion: string; enginePolicy: string; geoMode: GeoMode; timezone: string | null; locale: string | null;
  languages: string[]; geo: { lat: number; lon: number; accuracyM: number } | null; webrtcMode: WebRtcMode;
  dnsMode: string; ipv6Mode: string; blockLaunchOnMismatch: boolean; startUrls: string[]; extensions: string[];
  ramLimitMb: number | null; cpuLimitPct: number | null; launchPriority: number; passwordProtected: boolean; createdAt: number;
}
export interface ProfileDraft {
  name: string; folderId?: string | null; color?: string | null; notes?: string | null; proxyId?: string | null;
  geoMode?: GeoMode; timezone?: string | null; languages?: string[] | null; webrtcMode?: WebRtcMode;
  startUrls?: string[]; tagIds?: string[]; ramLimitMb?: number | null;
}
export type ProfilePatch = Partial<{
  name: string; folderId: string | null; color: string | null; notes: string | null; proxyId: string | null;
  geoMode: GeoMode; timezone: string | null; languages: string[]; webrtcMode: WebRtcMode; startUrls: string[];
  tagIds: string[]; pinned: boolean; ramLimitMb: number | null; blockLaunchOnMismatch: boolean;
}>;
export interface FilterQuery {
  text?: string; folderIds?: string[]; tagIdsAny?: string[]; statuses?: ProfileStatus[]; hasProxy?: boolean;
  inTrash?: boolean; pinned?: boolean;
  sort?: { field: 'name' | 'updatedAt' | 'createdAt' | 'lastLaunchedAt' | 'healthScore' | 'status'; dir: 'asc' | 'desc' };
}
export interface Page<T> { items: T[]; total: number; offset: number; limit: number }
export interface Folder { id: string; parentId: string | null; name: string; color: string | null; sortOrder: number; profileCount: number }
export interface Tag { id: string; name: string; color: string; count: number }
export interface Proxy {
  id: string; label: string | null; kind: ProxyKind; host: string | null; port: number | null; username: string | null;
  hasPassword: boolean; rotateUrl: string | null; lastStatus: ProxyStatus; lastLatencyMs: number | null; lastCheckAt: number | null;
  exitIp: string | null; exitCountry: string | null; exitCity: string | null; exitTimezone: string | null; exitAsn: number | null;
  exitIsp: string | null; profileCount: number;
}
export interface ProxyCheckResult { status: ProxyStatus; latencyMs: number | null; exitIp: string | null; country: string | null; city: string | null; timezone: string | null; isp: string | null; error: string | null }
export interface ParsedProxy { kind: ProxyKind; host: string; port: number; username: string | null; hasPassword: boolean }
export interface LogRow { id: number; ts: number; level: string; category: string; profileId: string | null; messageKey: string; params: any }
export interface VaultStatus { exists: boolean; unlocked: boolean; hint: string | null; retryAfterSecs: number; autolockMinutes: number | null }
export interface EngineInfo { engineVersion: string; engineMajor: number; enginePath: string; engineInstalled: boolean; host: { osFamily: string; osVersion: string; gpu: string; cores: number; memGb: number } }
export interface AppErr { code: string; detail?: any }

export function errCode(e: unknown): AppErr {
  if (e && typeof e === 'object' && 'code' in (e as any)) return e as AppErr;
  return { code: 'internal', detail: String(e) };
}

export const api = {
  vaultStatus: () => invoke<VaultStatus>('vault_status'),
  vaultCreate: (password: string, hint: string | null) => invoke<{ recoveryCode: string }>('vault_create', { password, hint }),
  vaultUnlock: (password: string) => invoke<void>('vault_unlock', { password }),
  vaultUnlockRecovery: (code: string) => invoke<void>('vault_unlock_recovery', { code }),
  vaultLock: () => invoke<void>('vault_lock'),
  vaultChangePassword: (old: string, next: string, hint: string | null) => invoke<void>('vault_change_password', { old, new: next, hint }),

  profilesQuery: (q: FilterQuery, page: { offset: number; limit: number }) => invoke<Page<ProfileRow>>('profiles_query', { q, page }),
  profilesIdsFor: (q: FilterQuery) => invoke<string[]>('profiles_ids_for', { q }),
  profileGet: (id: string) => invoke<ProfileDetail>('profile_get', { id }),
  profileCreate: (draft: ProfileDraft) => invoke<ProfileDetail>('profile_create', { draft }),
  profileUpdate: (id: string, patch: ProfilePatch) => invoke<ProfileDetail>('profile_update', { id, patch }),
  profilesTrash: (ids: string[]) => invoke<void>('profiles_trash', { ids }),
  profilesRestore: (ids: string[]) => invoke<void>('profiles_restore', { ids }),
  bulkCreate: (req: { count: number; namePattern: string; startIndex: number; draft: ProfileDraft }) => invoke<{ created: number }>('bulk_create', { req }),

  proxiesList: () => invoke<Proxy[]>('proxies_list'),
  proxiesParse: (text: string, defaultKind?: ProxyKind) => invoke<{ items: ParsedProxy[]; issues: { line: number; messageKey: string }[] }>('proxies_parse', { text, defaultKind }),
  proxiesImport: (text: string, defaultKind: ProxyKind) => invoke<number>('proxies_import', { text, defaultKind }),
  proxyUpsert: (draft: { id?: string; label?: string | null; kind: ProxyKind; host: string; port: number; username?: string | null; password?: string | null }) => invoke<Proxy>('proxy_upsert', { draft }),
  proxiesDelete: (ids: string[]) => invoke<void>('proxies_delete', { ids }),
  proxyCheck: (id: string) => invoke<ProxyCheckResult>('proxy_check', { id }),

  foldersList: () => invoke<Folder[]>('folders_list'),
  folderUpsert: (name: string, parentId: string | null = null, id: string | null = null) => invoke<Folder>('folder_upsert', { id, parentId, name, color: null }),
  folderDelete: (id: string) => invoke<void>('folder_delete', { id }),
  tagsList: () => invoke<Tag[]>('tags_list'),
  tagUpsert: (name: string, color: string) => invoke<string>('tag_upsert', { id: null, name, color }),
  tagDelete: (id: string) => invoke<void>('tag_delete', { id }),

  launch: (ids: string[]) => invoke<string[]>('launch_profiles', { ids }),
  stop: (ids: string[]) => invoke<void>('stop_profiles', { ids }),
  settingsGet: () => invoke<Record<string, any>>('settings_get'),
  settingSet: (key: string, value: unknown) => invoke<void>('setting_set', { key, value }),
  logsQuery: (category: string | null, page: { offset: number; limit: number }) => invoke<Page<LogRow>>('logs_query', { category, page }),
  engineInfo: () => invoke<EngineInfo>('engine_info'),
};
