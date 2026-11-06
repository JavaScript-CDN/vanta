import { useToasts } from '../store';

export function Toasts() {
  const { items, dismiss } = useToasts();
  return (
    <div className="toasts" aria-live="polite">
      {items.map((t) => <div key={t.id} className={`toast ${t.kind}`} onClick={() => dismiss(t.id)}>{t.text}</div>)}
    </div>
  );
}
