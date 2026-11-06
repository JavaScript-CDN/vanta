import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { api, FilterQuery, Folder, GeoMode, ProfileDetail, ProfileRow, Proxy, Tag, WebRtcMode } from '../api';
import { errText, useI18n } from '../i18n';
import { useSelection, useToasts, useUi } from '../store';
import { Modal } from '../components/Modal';

export type ProfileScope = { kind: 'all' } | { kind: 'trash' } | { kind: 'folder'; id: string } | { kind: 'tag'; id: string };

const PAGE = 200;
const ROW_H = 44;

function scopeQuery(scope: ProfileScope, text: string): FilterQuery {
  const q: FilterQuery = { text: text.trim() || undefined, sort: { field: 'updatedAt', dir: 'desc' } };
  if (scope.kind === 'trash') q.inTrash = true;
  if (scope.kind === 'folder') q.folderIds = [scope.id];
  if (scope.kind === 'tag') q.tagIdsAny = [scope.id];
  return q;
}

/** Windowed fetch cache: loads 200-row pages on demand so 10,000 rows never sit in memory at once. */
function useProfileWindow(query: FilterQuery, tick: number) {
  const [total, setTotal] = useState(0);
  const [pages, setPages] = useState<Record<number, ProfileRow[]>>({});
  const loading = useRef(new Set<number>());
  const key = JSON.stringify(query) + tick;
  useEffect(() => {
    let live = true;
    loading.current = new Set([0]);
    api.profilesQuery(query, { offset: 0, limit: PAGE }).then((p) => { if (live) { setTotal(p.total); setPages({ 0: p.items }); } }).catch(() => {});
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const ensure = useCallback((from: number, to: number) => {
    for (let pg = Math.floor(from / PAGE); pg <= Math.floor(to / PAGE); pg++) {
      if (pages[pg] || loading.current.has(pg)) continue;
      loading.current.add(pg);
      api.profilesQuery(query, { offset: pg * PAGE, limit: PAGE }).then((p) => setPages((s) => ({ ...s, [pg]: p.items }))).catch(() => loading.current.delete(pg));
    }
  }, [pages, query]);
  const get = (i: number) => pages[Math.floor(i / PAGE)]?.[i % PAGE];
  return { total, get, ensure };
}

export function ProfilesScreen({ scope, folders, tags, onOrgChanged }: { scope: ProfileScope; folders: Folder[]; tags: Tag[]; onOrgChanged: () => void }) {
  const { t } = useI18n();
  const push = useToasts((s) => s.push);
  const { refreshTick, bump } = useUi();
  const sel = useSelection();
  const [text, setText] = useState('');
  const [debounced, setDebounced] = useState('');
  useEffect(() => { const h = setTimeout(() => setDebounced(text), 200); return () => clearTimeout(h); }, [text]);
  const query = useMemo(() => scopeQuery(scope, debounced), [scope, debounced]);
  const { total, get, ensure } = useProfileWindow(query, refreshTick);
  const [editing, setEditing] = useState<ProfileDetail | 'new' | null>(null);
  const [bulk, setBulk] = useState(false);
  const [busy, setBusy] = useState(false);
  const parent = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({ count: total, getScrollElement: () => parent.current, estimateSize: () => ROW_H, overscan: 20 });
  const items = v.getVirtualItems();
  useEffect(() => { if (items.length) ensure(items[0].index, items[items.length - 1].index); }, [items, ensure]);

  // Poll while something is starting/running so status stays fresh.
  useEffect(() => { const iv = setInterval(() => bump(), 5000); return () => clearInterval(iv); }, [bump]);

  const selectedIds = async (): Promise<string[]> => sel.allMatching
    ? (await api.profilesIdsFor(query)).filter((id) => !sel.excluded.has(id))
    : [...sel.ids];

  const run = async (fn: (ids: string[]) => Promise<void>) => {
    setBusy(true);
    try { const ids = await selectedIds(); if (ids.length) await fn(ids); bump(); onOrgChanged(); }
    catch (e) { push('err', errText(t, e)); }
    finally { setBusy(false); }
  };

  const launch = (ids: string[]) => api.launch(ids).then((res) => {
    for (const r of res) {
      const [, code] = r.split(':');
      if (code === 'started') push('ok', t('launch.started'));
      else push('err', `${t('launch.failed')}: ${t(code.includes('.') ? code : `launch.${code}`)}`);
    }
  });

  const openEdit = async (id: string) => { try { setEditing(await api.profileGet(id)); } catch (e) { push('err', errText(t, e)); } };
  const count = sel.count(total);
  const inTrash = scope.kind === 'trash';

  return (
    <>
      <div className="toolbar">
        <input placeholder={t('profiles.search')} value={text} onChange={(e) => setText(e.target.value)} />
        <span className="muted small">{t('profiles.total', { n: total })}</span>
        <span className="grow" />
        {!inTrash && <button onClick={() => setBulk(true)}>{t('profiles.bulk')}</button>}
        {!inTrash && <button className="primary" onClick={() => setEditing('new')}>+ {t('profiles.new')}</button>}
      </div>
      {count > 0 && (
        <div className="bulkbar">
          <b>{t('profiles.selected', { n: count })}</b>
          {!sel.allMatching && total > sel.ids.size && <button className="ghost" onClick={sel.selectAll}>{t('profiles.select_all', { n: total })}</button>}
          <button className="ghost" onClick={sel.clear}>{t('profiles.clear')}</button>
          <span className="grow" />
          {inTrash ? (
            <button disabled={busy} onClick={() => run((ids) => api.profilesRestore(ids)).then(sel.clear)}>{t('profiles.restore')}</button>
          ) : (<>
            <button disabled={busy} className="primary" onClick={() => run(launch)}>▶ {t('profiles.launch')}</button>
            <button disabled={busy} onClick={() => run((ids) => api.stop(ids))}>■ {t('profiles.stop')}</button>
            <select style={{ width: 180 }} value="" onChange={(e) => { const f = e.target.value; run((ids) => Promise.all(ids.map((id) => api.profileUpdate(id, { folderId: f === '__none' ? null : f }))).then(() => {})); }}>
              <option value="" disabled>{t('profiles.move_to')}</option>
              <option value="__none">{t('profiles.no_folder')}</option>
              {folders.map((f) => <option key={f.id} value={f.id}>{f.name}</option>)}
            </select>
            <button disabled={busy} className="danger" onClick={() => { if (window.confirm(t('common.confirm_trash', { n: count }))) run((ids) => api.profilesTrash(ids)).then(sel.clear); }}>{t('profiles.trash')}</button>
          </>)}
        </div>
      )}
      <div className="table" ref={parent} role="grid" aria-rowcount={total}>
        <div className="thead">
          <input type="checkbox" aria-label="select all" checked={sel.allMatching && sel.excluded.size === 0 && total > 0} onChange={(e) => e.target.checked ? sel.selectAll() : sel.clear()} />
          <div>{t('col.name')}</div><div>{t('col.status')}</div><div>{t('col.os')}</div><div>{t('col.proxy')}</div><div>{t('col.tags')}</div><div>{t('col.last')}</div><div />
        </div>
        {total === 0 ? <div className="empty">{t('profiles.empty')}</div> : (
          <div style={{ height: v.getTotalSize(), position: 'relative' }}>
            {items.map((vi) => {
              const row = get(vi.index);
              const style = { position: 'absolute' as const, top: 0, left: 0, right: 0, transform: `translateY(${vi.start}px)` };
              if (!row) return <div key={`ph-${vi.index}`} className="trow" style={style}><span /><span className="muted">…</span></div>;
              const selected = sel.isSelected(row.id);
              const running = row.status === 'running' || row.status === 'starting';
              return (
                <div key={row.id} className={`trow ${selected ? 'sel' : ''}`} style={style}>
                  <input type="checkbox" checked={selected} onChange={() => sel.toggle(row.id)} aria-label={row.name} />
                  <div className="name" onDoubleClick={() => openEdit(row.id)} onClick={() => openEdit(row.id)} title={row.notesPreview ?? ''}>
                    {row.pinned ? '📌 ' : ''}{row.color && <span className="dot" style={{ background: row.color, marginRight: 6 }} />}{row.name}
                  </div>
                  <div><span className={`badge ${row.status}`}>{t(`status.${row.status}`)}</span></div>
                  <div className="cell muted">{osLabel(row)}</div>
                  <div className="cell">{row.proxy ? <><span className={`badge ${row.proxy.status}`}>{row.proxy.country ?? '—'}</span> <span className="muted small">{row.proxy.label ?? ''}</span></> : <span className="muted">—</span>}</div>
                  <div className="cell">{row.tags.map((g) => <span key={g.id} className="tag" style={{ borderColor: g.color }}>{g.name}</span>)}</div>
                  <div className="cell muted small">{row.lastLaunchedAt ? new Date(row.lastLaunchedAt).toLocaleString() : '—'}</div>
                  <div className="row">
                    {!inTrash && (running
                      ? <button onClick={() => api.stop([row.id]).then(bump).catch((e) => push('err', errText(t, e)))}>■ {t('profiles.stop')}</button>
                      : <button className="primary" onClick={() => launch([row.id]).then(bump).catch((e) => push('err', errText(t, e)))}>▶ {t('profiles.launch')}</button>)}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
      {editing && <ProfileEditor initial={editing === 'new' ? null : editing} folders={folders} tags={tags} defaultFolder={scope.kind === 'folder' ? scope.id : null}
        onClose={() => setEditing(null)} onSaved={() => { setEditing(null); bump(); onOrgChanged(); }} />}
      {bulk && <BulkCreate folders={folders} defaultFolder={scope.kind === 'folder' ? scope.id : null} onClose={() => setBulk(false)} onDone={() => { setBulk(false); bump(); onOrgChanged(); }} />}
    </>
  );
}

function osLabel(r: ProfileRow) {
  if (r.osFamily === 'windows') {
    const build = Number(r.osVersion.split('.').pop());
    return `${build >= 22000 ? 'Windows 11' : 'Windows 10'} · Chrome ${r.browserMajor}`;
  }
  return `${r.osFamily} · Chrome ${r.browserMajor}`;
}

function useProxies() {
  const [list, setList] = useState<Proxy[]>([]);
  useEffect(() => { api.proxiesList().then(setList).catch(() => {}); }, []);
  return list;
}

function ProfileEditor({ initial, folders, tags, defaultFolder, onClose, onSaved }: {
  initial: ProfileDetail | null; folders: Folder[]; tags: Tag[]; defaultFolder: string | null; onClose: () => void; onSaved: () => void;
}) {
  const { t } = useI18n();
  const push = useToasts((s) => s.push);
  const proxies = useProxies();
  const [name, setName] = useState(initial?.name ?? '');
  const [folderId, setFolderId] = useState<string | null>(initial?.folderId ?? defaultFolder);
  const [tagIds, setTagIds] = useState<string[]>(initial?.tags.map((g) => g.id) ?? []);
  const [proxyId, setProxyId] = useState<string | null>(initial?.proxy?.id ?? null);
  const [geoMode, setGeoMode] = useState<GeoMode>(initial?.geoMode ?? 'auto_from_proxy');
  const [timezone, setTimezone] = useState(initial?.timezone ?? '');
  const [languages, setLanguages] = useState((initial?.languages ?? []).join(', '));
  const [webrtc, setWebrtc] = useState<WebRtcMode>(initial?.webrtcMode ?? 'proxied');
  const [block, setBlock] = useState(initial?.blockLaunchOnMismatch ?? true);
  const [urls, setUrls] = useState((initial?.startUrls ?? []).join('\n'));
  const [notes, setNotes] = useState(initial?.notes ?? '');
  const [ram, setRam] = useState(initial?.ramLimitMb ? String(initial.ramLimitMb) : '');
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const langs = languages.split(',').map((s) => s.trim()).filter(Boolean);
  const startUrls = urls.split('\n').map((s) => s.trim()).filter(Boolean);
  const ramMb = ram.trim() ? Math.max(256, Number(ram)) : null;

  const save = async () => {
    setErr(null);
    if (!name.trim()) return setErr(t('err.validation'));
    setBusy(true);
    try {
      const geo = geoMode === 'manual' ? { timezone: timezone.trim() || null, languages: langs.length ? langs : null } : {};
      if (initial) {
        await api.profileUpdate(initial.id, { name: name.trim(), folderId, tagIds, proxyId, geoMode, webrtcMode: webrtc, startUrls, notes: notes || null,
          blockLaunchOnMismatch: block, ramLimitMb: ramMb, ...(geoMode === 'manual' ? { timezone: timezone.trim() || null, languages: langs } : {}) });
      } else {
        await api.profileCreate({ name: name.trim(), folderId, tagIds, proxyId, geoMode, webrtcMode: webrtc, startUrls, notes: notes || null, ramLimitMb: ramMb, ...geo });
      }
      onSaved();
    } catch (e) { setErr(errText(t, e)); push('err', errText(t, e)); }
    finally { setBusy(false); }
  };

  const r = initial?.resolved;
  return (
    <Modal wide title={initial ? t('editor.edit_title') : t('editor.create_title')} onClose={onClose}
      footer={<><button onClick={onClose}>{t('editor.cancel')}</button><button className="primary" disabled={busy} onClick={save}>{initial ? t('editor.save') : t('editor.create')}</button></>}>
      <div className="cols">
        <div>
          <label>{t('editor.name')}</label>
          <input autoFocus value={name} onChange={(e) => setName(e.target.value)} maxLength={120} />
          <label>{t('editor.folder')}</label>
          <select value={folderId ?? ''} onChange={(e) => setFolderId(e.target.value || null)}>
            <option value="">{t('profiles.no_folder')}</option>
            {folders.map((f) => <option key={f.id} value={f.id}>{f.name}</option>)}
          </select>
          <label>{t('editor.tags')}</label>
          <div className="row" style={{ flexWrap: 'wrap' }}>
            {tags.map((g) => (
              <button key={g.id} type="button" className={tagIds.includes(g.id) ? 'primary' : ''} style={{ padding: '3px 10px' }}
                onClick={() => setTagIds(tagIds.includes(g.id) ? tagIds.filter((x) => x !== g.id) : [...tagIds, g.id])}>{g.name}</button>
            ))}
            {tags.length === 0 && <span className="muted small">—</span>}
          </div>
          <label>{t('editor.proxy')}</label>
          <select value={proxyId ?? ''} onChange={(e) => setProxyId(e.target.value || null)}>
            <option value="">{t('editor.none')}</option>
            {proxies.map((p) => <option key={p.id} value={p.id}>{p.label || `${p.kind}://${p.host}:${p.port}`}{p.exitCountry ? ` · ${p.exitCountry}` : ''} · {t(`pstatus.${p.lastStatus}`)}</option>)}
          </select>
          <label>{t('editor.geo')}</label>
          <select value={geoMode} onChange={(e) => setGeoMode(e.target.value as GeoMode)}>
            {(['auto_from_proxy', 'manual', 'host'] as GeoMode[]).map((g) => <option key={g} value={g}>{t(`geo.${g}`)}</option>)}
          </select>
          {geoMode === 'manual' && <>
            <label>{t('editor.timezone')}</label>
            <input value={timezone} onChange={(e) => setTimezone(e.target.value)} placeholder="Asia/Dhaka" />
            <label>{t('editor.languages')}</label>
            <input value={languages} onChange={(e) => setLanguages(e.target.value)} placeholder="bn-BD, bn, en-US, en" />
          </>}
          <label>{t('editor.webrtc')}</label>
          <select value={webrtc} onChange={(e) => setWebrtc(e.target.value as WebRtcMode)}>
            {(['proxied', 'public_only', 'disabled_present', 'real'] as WebRtcMode[]).map((w) => <option key={w} value={w}>{t(`webrtc.${w}`)}</option>)}
          </select>
          <label className="row" style={{ color: 'var(--text)' }}><input type="checkbox" style={{ width: 'auto' }} checked={block} onChange={(e) => setBlock(e.target.checked)} /> {t('editor.block_mismatch')}</label>
        </div>
        <div>
          <label>{t('editor.start_urls')}</label>
          <textarea value={urls} onChange={(e) => setUrls(e.target.value)} placeholder="https://" />
          <label>{t('editor.notes')}</label>
          <textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
          <label>{t('editor.ram')}</label>
          <input inputMode="numeric" value={ram} onChange={(e) => setRam(e.target.value.replace(/\D/g, ''))} />
          {r && (
            <div className="section">
              <h3>{t('editor.resolved')}</h3>
              <p className="muted small" style={{ marginTop: 0 }}>{t('editor.native_note')}</p>
              <div className="kv mono small">
                <div>User-Agent</div><div style={{ wordBreak: 'break-all' }}>{r.userAgent}</div>
                <div>Platform</div><div>{r.navigator?.platform} · {r.uaCh?.platform}</div>
                <div>CPU / RAM</div><div>{r.navigator?.hardwareConcurrency} cores · {r.navigator?.deviceMemory} GB</div>
                <div>Screen</div><div>{r.screen?.width}×{r.screen?.height} @{r.screen?.dpr}x</div>
                <div>Languages</div><div>{r.acceptLanguage}</div>
                <div>Timezone</div><div>{r.timezone ?? '—'}</div>
                <div>Media devices</div><div>{r.mediaDevices?.audioinput} mic · {r.mediaDevices?.audiooutput} out · {r.mediaDevices?.videoinput} cam</div>
                <div>Engine</div><div>{initial?.engineVersion}</div>
              </div>
            </div>
          )}
        </div>
      </div>
      {err && <div className="err">{err}</div>}
    </Modal>
  );
}

function BulkCreate({ folders, defaultFolder, onClose, onDone }: { folders: Folder[]; defaultFolder: string | null; onClose: () => void; onDone: () => void }) {
  const { t } = useI18n();
  const push = useToasts((s) => s.push);
  const [count, setCount] = useState('10');
  const [pattern, setPattern] = useState('Profile-{n:000}');
  const [start, setStart] = useState('1');
  const [folderId, setFolderId] = useState<string | null>(defaultFolder);
  const [busy, setBusy] = useState(false);
  const n = Math.min(10000, Math.max(1, Number(count) || 1));
  const go = async () => {
    setBusy(true);
    try {
      const r = await api.bulkCreate({ count: n, namePattern: pattern, startIndex: Number(start) || 1, draft: { name: '', folderId, geoMode: 'auto_from_proxy', webrtcMode: 'proxied' } });
      push('ok', t('bulk.done', { n: r.created }));
      onDone();
    } catch (e) { push('err', errText(t, e)); }
    finally { setBusy(false); }
  };
  return (
    <Modal title={t('bulk.title')} onClose={onClose}
      footer={<><button onClick={onClose}>{t('editor.cancel')}</button><button className="primary" disabled={busy} onClick={go}>{busy ? t('bulk.working') : t('bulk.create', { n })}</button></>}>
      <label>{t('bulk.count')}</label>
      <input inputMode="numeric" value={count} onChange={(e) => setCount(e.target.value.replace(/\D/g, ''))} />
      <label>{t('bulk.pattern')}</label>
      <input value={pattern} onChange={(e) => setPattern(e.target.value)} />
      <label>{t('bulk.start')}</label>
      <input inputMode="numeric" value={start} onChange={(e) => setStart(e.target.value.replace(/\D/g, ''))} />
      <label>{t('editor.folder')}</label>
      <select value={folderId ?? ''} onChange={(e) => setFolderId(e.target.value || null)}>
        <option value="">{t('profiles.no_folder')}</option>
        {folders.map((f) => <option key={f.id} value={f.id}>{f.name}</option>)}
      </select>
    </Modal>
  );
}
