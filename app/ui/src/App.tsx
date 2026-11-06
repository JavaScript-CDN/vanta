import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api, Folder, Tag, VaultStatus } from './api';
import { errText, useI18n } from './i18n';
import { useSelection, useToasts, useUi } from './store';
import { Toasts } from './components/Toasts';
import { ProfilesScreen, ProfileScope } from './screens/Profiles';
import { ProxiesScreen } from './screens/Proxies';
import { SettingsScreen } from './screens/Settings';
import { LogsScreen } from './screens/Logs';

type Route = 'profiles' | 'proxies' | 'logs' | 'settings';

export default function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const refresh = useCallback(() => api.vaultStatus().then(setStatus).catch(() => setStatus(null)), []);
  useEffect(() => { refresh(); }, [refresh]);
  useI18n((s) => s.lang); // re-render on language change
  if (!status) return <div className="gate"><div className="muted">…</div></div>;
  return (
    <>
      {status.unlocked ? <Shell onLocked={refresh} autolock={status.autolockMinutes} /> : <VaultGate status={status} onUnlocked={refresh} />}
      <Toasts />
    </>
  );
}

function VaultGate({ status, onUnlocked }: { status: VaultStatus; onUnlocked: () => void }) {
  const { t } = useI18n();
  const [pw, setPw] = useState('');
  const [pw2, setPw2] = useState('');
  const [hint, setHint] = useState('');
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [recovery, setRecovery] = useState<string | null>(null);
  const [useRecovery, setUseRecovery] = useState(false);

  const create = async () => {
    setErr(null);
    if (pw.length < 10) return setErr(t('vault.password_too_short'));
    if (pw !== pw2) return setErr(t('vault.mismatch'));
    setBusy(true);
    try { const r = await api.vaultCreate(pw, hint.trim() || null); setPw(''); setPw2(''); setRecovery(r.recoveryCode); }
    catch (e) { setErr(errText(t, e)); }
    finally { setBusy(false); }
  };
  const unlock = async () => {
    setErr(null); setBusy(true);
    try { if (useRecovery) await api.vaultUnlockRecovery(pw); else await api.vaultUnlock(pw); setPw(''); onUnlocked(); }
    catch (e) { setErr(errText(t, e)); }
    finally { setBusy(false); }
  };

  if (recovery) {
    return (
      <div className="gate"><div className="card">
        <div className="brand"><span className="logo">V</span>{t('vault.recovery_title')}</div>
        <p className="muted small">{t('vault.recovery_note')}</p>
        <div className="code">{recovery}</div>
        <div className="row" style={{ marginTop: 16 }}><span className="grow" /><button className="primary" onClick={() => { setRecovery(null); onUnlocked(); }}>{t('vault.recovery_done')}</button></div>
      </div></div>
    );
  }

  const creating = !status.exists;
  return (
    <div className="gate"><form className="card" onSubmit={(e) => { e.preventDefault(); creating ? create() : unlock(); }}>
      <div className="brand"><span className="logo">V</span>{creating ? t('vault.welcome') : t('vault.unlock_title')}</div>
      {creating && <p className="muted small">{t('vault.welcome_sub')}</p>}
      {!creating && status.hint && !useRecovery && <p className="muted small">{t('vault.hint_label', { h: status.hint })}</p>}
      <label>{useRecovery ? t('vault.recovery_code') : t('vault.password')}</label>
      <input type={useRecovery ? 'text' : 'password'} autoFocus value={pw} onChange={(e) => setPw(e.target.value)} autoComplete="off" spellCheck={false} />
      {creating && <>
        <label>{t('vault.confirm')}</label>
        <input type="password" value={pw2} onChange={(e) => setPw2(e.target.value)} autoComplete="off" />
        <label>{t('vault.hint')}</label>
        <input value={hint} onChange={(e) => setHint(e.target.value)} maxLength={80} />
      </>}
      {err && <div className="err">{err}</div>}
      <div className="row" style={{ marginTop: 18 }}>
        {!creating && <button type="button" className="ghost small" onClick={() => { setUseRecovery(!useRecovery); setPw(''); setErr(null); }}>{useRecovery ? t('vault.use_password') : t('vault.use_recovery')}</button>}
        <span className="grow" />
        <button className="primary" disabled={busy || !pw}>{busy ? t('vault.deriving') : creating ? t('vault.create') : t('vault.unlock')}</button>
      </div>
    </form></div>
  );
}

function Shell({ onLocked, autolock }: { onLocked: () => void; autolock: number | null }) {
  const { t } = useI18n();
  const push = useToasts((s) => s.push);
  const { bump, refreshTick } = useUi();
  const [route, setRoute] = useState<Route>('profiles');
  const [scope, setScope] = useState<ProfileScope>({ kind: 'all' });
  const [folders, setFolders] = useState<Folder[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const clearSel = useSelection((s) => s.clear);

  const loadOrg = useCallback(() => { api.foldersList().then(setFolders).catch(() => {}); api.tagsList().then(setTags).catch(() => {}); }, []);
  useEffect(() => { loadOrg(); }, [loadOrg, refreshTick]);

  // Engine exits/crashes come from the core as events.
  useEffect(() => {
    const un = listen<{ id: string; status: string }>('profile:state', () => bump());
    return () => { un.then((f) => f()); };
  }, [bump]);

  const lock = useCallback(async () => { await api.vaultLock().catch(() => {}); onLocked(); }, [onLocked]);

  // Auto-lock after N idle minutes (file 02 §7.7).
  const last = useRef(Date.now());
  useEffect(() => {
    if (!autolock) return;
    const mark = () => { last.current = Date.now(); };
    const evs = ['mousemove', 'keydown', 'mousedown', 'wheel'];
    evs.forEach((e) => window.addEventListener(e, mark, { passive: true }));
    const iv = setInterval(() => { if (Date.now() - last.current > autolock * 60_000) lock(); }, 15_000);
    return () => { evs.forEach((e) => window.removeEventListener(e, mark)); clearInterval(iv); };
  }, [autolock, lock]);

  const go = (r: Route, s?: ProfileScope) => { setRoute(r); if (s) { setScope(s); clearSel(); } };
  const newFolder = async () => {
    const name = window.prompt(t('nav.new_folder'));
    if (!name) return;
    try { await api.folderUpsert(name); loadOrg(); } catch (e) { push('err', errText(t, e)); }
  };
  const newTag = async () => {
    const name = window.prompt(t('nav.new_tag'));
    if (!name) return;
    const palette = ['#38d6b4', '#7aa2ff', '#f5b84a', '#ff7aa8', '#b38cff', '#5cc8ff'];
    try { await api.tagUpsert(name, palette[tags.length % palette.length]); loadOrg(); } catch (e) { push('err', errText(t, e)); }
  };
  const isScope = (s: ProfileScope) => route === 'profiles' && JSON.stringify(s) === JSON.stringify(scope);

  return (
    <div className="shell">
      <aside className="side">
        <div className="brand"><span className="logo">V</span>Vanta</div>
        <button className={`nav ${isScope({ kind: 'all' }) ? 'active' : ''}`} onClick={() => go('profiles', { kind: 'all' })}>{t('nav.all')}</button>
        <button className={`nav ${route === 'proxies' ? 'active' : ''}`} onClick={() => go('proxies')}>{t('nav.proxies')}</button>
        <button className={`nav ${isScope({ kind: 'trash' }) ? 'active' : ''}`} onClick={() => go('profiles', { kind: 'trash' })}>{t('nav.trash')}</button>
        <h4>{t('nav.folders')}<button className="ghost" onClick={newFolder} title={t('nav.new_folder')}>+</button></h4>
        {folders.map((f) => (
          <button key={f.id} className={`nav ${isScope({ kind: 'folder', id: f.id }) ? 'active' : ''}`} onClick={() => go('profiles', { kind: 'folder', id: f.id })}
            onContextMenu={async (e) => { e.preventDefault(); if (window.confirm(`${f.name}: ${t('proxies.delete')}?`)) { await api.folderDelete(f.id).catch(() => {}); loadOrg(); } }}>
            📁 <span className="cell">{f.name}</span><span className="count">{f.profileCount}</span>
          </button>
        ))}
        <h4>{t('nav.tags')}<button className="ghost" onClick={newTag} title={t('nav.new_tag')}>+</button></h4>
        {tags.map((g) => (
          <button key={g.id} className={`nav ${isScope({ kind: 'tag', id: g.id }) ? 'active' : ''}`} onClick={() => go('profiles', { kind: 'tag', id: g.id })}
            onContextMenu={async (e) => { e.preventDefault(); if (window.confirm(`${g.name}: ${t('proxies.delete')}?`)) { await api.tagDelete(g.id).catch(() => {}); loadOrg(); } }}>
            <span className="dot" style={{ background: g.color }} /> <span className="cell">{g.name}</span><span className="count">{g.count}</span>
          </button>
        ))}
        <span className="grow" />
        <button className={`nav ${route === 'logs' ? 'active' : ''}`} onClick={() => go('logs')}>{t('nav.logs')}</button>
        <button className={`nav ${route === 'settings' ? 'active' : ''}`} onClick={() => go('settings')}>{t('nav.settings')}</button>
        <button className="nav" onClick={lock}>🔒 {t('vault.lock')}</button>
      </aside>
      <main className="main">
        {route === 'profiles' && <ProfilesScreen scope={scope} folders={folders} tags={tags} onOrgChanged={loadOrg} />}
        {route === 'proxies' && <ProxiesScreen />}
        {route === 'logs' && <LogsScreen />}
        {route === 'settings' && <SettingsScreen onLocked={onLocked} />}
      </main>
    </div>
  );
}
