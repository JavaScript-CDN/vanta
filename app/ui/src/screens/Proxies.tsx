import { useCallback, useEffect, useState } from 'react';
import { api, ParsedProxy, Proxy, ProxyKind } from '../api';
import { errText, useI18n } from '../i18n';
import { useToasts } from '../store';
import { Modal } from '../components/Modal';

export function ProxiesScreen() {
  const { t } = useI18n();
  const push = useToasts((s) => s.push);
  const [list, setList] = useState<Proxy[]>([]);
  const [checking, setChecking] = useState<Set<string>>(new Set());
  const [importing, setImporting] = useState(false);
  const [adding, setAdding] = useState(false);
  const load = useCallback(() => api.proxiesList().then(setList).catch((e) => push('err', errText(t, e))), [push, t]);
  useEffect(() => { load(); }, [load]);

  const check = async (ids: string[]) => {
    setChecking((s) => new Set([...s, ...ids]));
    // Bounded parallelism: 8 checks at a time.
    const queue = [...ids];
    const worker = async () => {
      while (queue.length) {
        const id = queue.shift()!;
        try { await api.proxyCheck(id); } catch (e) { push('err', errText(t, e)); }
        setChecking((s) => { const n = new Set(s); n.delete(id); return n; });
        load();
      }
    };
    await Promise.all(Array.from({ length: Math.min(8, ids.length) }, worker));
  };

  return (
    <>
      <div className="toolbar">
        <b>{t('proxies.title')}</b><span className="muted small">{list.length}</span>
        <span className="grow" />
        <button disabled={!list.length || checking.size > 0} onClick={() => check(list.map((p) => p.id))}>{t('proxies.check_all')}</button>
        <button onClick={() => setImporting(true)}>{t('proxies.import')}</button>
        <button className="primary" onClick={() => setAdding(true)}>+ {t('proxies.add')}</button>
      </div>
      <div className="content">
        {list.length === 0 ? <div className="empty">{t('proxies.empty')}</div> : (
          <table className="ptable">
            <thead><tr>
              <th>{t('proxies.label')}</th><th>{t('proxies.kind')}</th><th>{t('proxies.host')}</th><th>{t('col.status')}</th>
              <th>{t('proxies.exit')}</th><th>{t('proxies.latency')}</th><th>{t('proxies.used_by')}</th><th />
            </tr></thead>
            <tbody>
              {list.map((p) => (
                <tr key={p.id}>
                  <td>{p.label ?? '—'}</td>
                  <td className="mono small">{p.kind}</td>
                  <td className="mono small">{p.host}:{p.port}{p.username ? ` · ${p.username}` : ''}{p.hasPassword ? ' 🔑' : ''}</td>
                  <td><span className={`badge ${p.lastStatus}`}>{checking.has(p.id) ? '…' : t(`pstatus.${p.lastStatus}`)}</span></td>
                  <td className="small">{p.exitIp ? <>{p.exitCountry} · {p.exitCity}<div className="muted mono">{p.exitIp} · {p.exitTimezone}</div></> : '—'}</td>
                  <td className="small">{p.lastLatencyMs != null ? `${p.lastLatencyMs} ms` : '—'}</td>
                  <td className="small">{p.profileCount}</td>
                  <td className="row">
                    <button disabled={checking.has(p.id)} onClick={() => check([p.id])}>{t('proxies.check')}</button>
                    <button className="danger" onClick={async () => { if (window.confirm(`${t('proxies.delete')}?`)) { await api.proxiesDelete([p.id]).catch((e) => push('err', errText(t, e))); load(); } }}>✕</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {importing && <ImportDialog onClose={() => setImporting(false)} onDone={() => { setImporting(false); load(); }} />}
      {adding && <AddDialog onClose={() => setAdding(false)} onDone={() => { setAdding(false); load(); }} />}
    </>
  );
}

function ImportDialog({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const { t } = useI18n();
  const push = useToasts((s) => s.push);
  const [text, setText] = useState('');
  const [kind, setKind] = useState<ProxyKind>('http');
  const [preview, setPreview] = useState<{ items: ParsedProxy[]; issues: { line: number; messageKey: string }[] } | null>(null);
  useEffect(() => {
    const h = setTimeout(() => { if (text.trim()) api.proxiesParse(text, kind).then(setPreview).catch(() => setPreview(null)); else setPreview(null); }, 250);
    return () => clearTimeout(h);
  }, [text, kind]);
  const go = async () => {
    try { const n = await api.proxiesImport(text, kind); setText(''); push('ok', t('proxies.imported', { n })); onDone(); }
    catch (e) { push('err', errText(t, e)); }
  };
  return (
    <Modal title={t('proxies.import')} onClose={onClose}
      footer={<><button onClick={onClose}>{t('editor.cancel')}</button><button className="primary" disabled={!preview?.items.length} onClick={go}>{t('proxies.import')}</button></>}>
      <label>{t('proxies.paste')}</label>
      <textarea className="mono" style={{ minHeight: 180 }} value={text} onChange={(e) => setText(e.target.value)} spellCheck={false} autoComplete="off" />
      <label>{t('proxies.default_kind')}</label>
      <select value={kind} onChange={(e) => setKind(e.target.value as ProxyKind)}>
        <option value="http">HTTP</option><option value="https">HTTPS (CONNECT)</option><option value="socks5">SOCKS5</option>
      </select>
      {preview && <p className="small">{t('proxies.preview', { ok: preview.items.length, bad: preview.issues.length })}
        {preview.issues.slice(0, 5).map((i) => <span key={i.line} className="err" style={{ display: 'block' }}>#{i.line}: {t(i.messageKey)}</span>)}</p>}
    </Modal>
  );
}

function AddDialog({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const { t } = useI18n();
  const push = useToasts((s) => s.push);
  const [kind, setKind] = useState<ProxyKind>('http');
  const [label, setLabel] = useState('');
  const [host, setHost] = useState('');
  const [port, setPort] = useState('');
  const [user, setUser] = useState('');
  const [pass, setPass] = useState('');
  const go = async () => {
    try {
      await api.proxyUpsert({ kind, label: label || null, host: host.trim(), port: Number(port), username: user || null, password: pass || null });
      setPass(''); onDone();
    } catch (e) { push('err', errText(t, e)); }
  };
  return (
    <Modal title={t('proxies.add')} onClose={onClose}
      footer={<><button onClick={onClose}>{t('editor.cancel')}</button><button className="primary" disabled={!host || !port} onClick={go}>{t('editor.save')}</button></>}>
      <label>{t('proxies.label')}</label><input value={label} onChange={(e) => setLabel(e.target.value)} />
      <label>{t('proxies.kind')}</label>
      <select value={kind} onChange={(e) => setKind(e.target.value as ProxyKind)}>
        <option value="http">HTTP</option><option value="https">HTTPS (CONNECT)</option><option value="socks5">SOCKS5</option>
      </select>
      <div className="row"><div className="grow"><label>{t('proxies.host')}</label><input value={host} onChange={(e) => setHost(e.target.value)} /></div>
        <div style={{ width: 120 }}><label>{t('proxies.port')}</label><input inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, '').slice(0, 5))} /></div></div>
      <div className="row"><div className="grow"><label>{t('proxies.user')}</label><input value={user} onChange={(e) => setUser(e.target.value)} autoComplete="off" /></div>
        <div className="grow"><label>{t('proxies.pass')}</label><input type="password" value={pass} onChange={(e) => setPass(e.target.value)} autoComplete="new-password" /></div></div>
    </Modal>
  );
}
