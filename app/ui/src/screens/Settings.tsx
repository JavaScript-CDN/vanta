import { useEffect, useState } from 'react';
import { api, EngineInfo } from '../api';
import { errText, Lang, useI18n } from '../i18n';
import { useToasts, useUi } from '../store';

export function SettingsScreen({ onLocked }: { onLocked: () => void }) {
  const { t, lang, setLang } = useI18n();
  const { theme, setTheme } = useUi();
  const push = useToasts((s) => s.push);
  const [info, setInfo] = useState<EngineInfo | null>(null);
  const [enginePath, setEnginePath] = useState('');
  const [maxC, setMaxC] = useState('');
  const [autolock, setAutolock] = useState('');
  const [oldPw, setOldPw] = useState('');
  const [newPw, setNewPw] = useState('');

  const load = () => {
    api.engineInfo().then(setInfo).catch(() => {});
    api.settingsGet().then((s) => {
      setEnginePath(s.enginePath ?? ''); setMaxC(s.maxConcurrent != null ? String(s.maxConcurrent) : '');
      setAutolock(s.autolock_minutes != null ? String(s.autolock_minutes) : '');
    }).catch(() => {});
  };
  useEffect(load, []);

  const save = async (key: string, value: unknown) => {
    try { await api.settingSet(key, value); push('ok', t('settings.saved')); load(); } catch (e) { push('err', errText(t, e)); }
  };
  const num = (s: string) => (s.trim() ? Number(s) : null);

  return (
    <>
      <div className="toolbar"><b>{t('settings.title')}</b></div>
      <div className="content">
        <div className="card2">
          <h2>{t('settings.engine')}</h2>
          {info && <p className="small">{info.engineInstalled ? `✅ ${t('settings.engine_ok')}` : `⚠️ ${t('settings.engine_missing')}`} · Chromium {info.engineVersion}<br /><span className="muted mono">{info.enginePath}</span></p>}
          <label>{t('settings.engine_path')}</label>
          <div className="row"><input className="mono" value={enginePath} onChange={(e) => setEnginePath(e.target.value)} placeholder="C:\Vanta\engine\chrome.exe" />
            <button onClick={() => save('enginePath', enginePath.trim() || null)}>{t('editor.save')}</button></div>
          <label>{t('settings.concurrency')}</label>
          <div className="row"><input inputMode="numeric" value={maxC} onChange={(e) => setMaxC(e.target.value.replace(/\D/g, ''))} />
            <button onClick={() => save('maxConcurrent', num(maxC))}>{t('editor.save')}</button></div>
        </div>
        <div className="card2">
          <h2>{t('settings.language')} / {t('settings.theme')}</h2>
          <div className="row">
            <select value={lang} onChange={(e) => setLang(e.target.value as Lang)}><option value="bn">বাংলা</option><option value="en">English</option></select>
            <select value={theme} onChange={(e) => setTheme(e.target.value as 'dark' | 'light')}><option value="dark">{t('theme.dark')}</option><option value="light">{t('theme.light')}</option></select>
          </div>
        </div>
        <div className="card2">
          <h2>{t('settings.security')}</h2>
          <label>{t('settings.autolock')}</label>
          <div className="row"><input inputMode="numeric" value={autolock} onChange={(e) => setAutolock(e.target.value.replace(/\D/g, ''))} />
            <button onClick={() => save('autolock_minutes', num(autolock))}>{t('editor.save')}</button></div>
          <label>{t('vault.change_pw')}</label>
          <div className="row">
            <input type="password" placeholder={t('vault.old_pw')} value={oldPw} onChange={(e) => setOldPw(e.target.value)} autoComplete="current-password" />
            <input type="password" placeholder={t('vault.new_pw')} value={newPw} onChange={(e) => setNewPw(e.target.value)} autoComplete="new-password" />
            <button disabled={!oldPw || newPw.length < 10} onClick={async () => {
              try { await api.vaultChangePassword(oldPw, newPw, null); setOldPw(''); setNewPw(''); push('ok', t('settings.saved')); }
              catch (e) { push('err', errText(t, e)); }
            }}>{t('editor.save')}</button>
          </div>
          <div className="row" style={{ marginTop: 12 }}><button onClick={async () => { await api.vaultLock().catch(() => {}); onLocked(); }}>🔒 {t('vault.lock')}</button></div>
        </div>
        {info && <div className="card2">
          <h2>{t('settings.host')}</h2>
          <div className="kv small"><div>OS</div><div>{info.host.osFamily} {info.host.osVersion}</div><div>GPU</div><div>{info.host.gpu}</div>
            <div>CPU</div><div>{info.host.cores}</div><div>RAM</div><div>{info.host.memGb} GB</div></div>
        </div>}
      </div>
    </>
  );
}
