import { useEffect, useState } from 'react';
import { api, LogRow } from '../api';
import { useI18n } from '../i18n';

export function LogsScreen() {
  const { t } = useI18n();
  const [rows, setRows] = useState<LogRow[]>([]);
  const [cat, setCat] = useState<string>('');
  const load = () => api.logsQuery(cat || null, { offset: 0, limit: 300 }).then((p) => setRows(p.items)).catch(() => {});
  useEffect(() => { load(); }, [cat]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <>
      <div className="toolbar">
        <b>{t('logs.title')}</b><span className="grow" />
        <select style={{ width: 160 }} value={cat} onChange={(e) => setCat(e.target.value)}>
          <option value="">*</option>{['launch', 'proxy', 'vault', 'import'].map((c) => <option key={c} value={c}>{c}</option>)}
        </select>
        <button onClick={load}>{t('common.refresh')}</button>
      </div>
      <div className="content">
        {rows.length === 0 ? <div className="empty">{t('logs.empty')}</div> : (
          <table className="ptable"><tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="muted small mono" style={{ width: 170 }}>{new Date(r.ts).toLocaleString()}</td>
                <td><span className={`badge ${r.level === 'error' ? 'fail' : 'ok'}`}>{r.level}</span></td>
                <td className="small">{r.category}</td>
                <td>{t(r.messageKey)} {r.params && Object.keys(r.params).length ? <span className="muted small mono">{JSON.stringify(r.params)}</span> : null}</td>
              </tr>
            ))}
          </tbody></table>
        )}
      </div>
    </>
  );
}
