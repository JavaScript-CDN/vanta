// Selection (file 04 §7.3: explicit ids OR "all matching minus exclusions") + toasts + theme.
import { create } from 'zustand';

interface Selection {
  ids: Set<string>; allMatching: boolean; excluded: Set<string>;
  toggle: (id: string) => void; setOnly: (id: string) => void; clear: () => void; selectAll: () => void;
  isSelected: (id: string) => boolean; count: (total: number) => number;
}
export const useSelection = create<Selection>((set, get) => ({
  ids: new Set(), allMatching: false, excluded: new Set(),
  toggle: (id) => set((s) => {
    if (s.allMatching) { const ex = new Set(s.excluded); if (ex.has(id)) ex.delete(id); else ex.add(id); return { excluded: ex }; }
    const ids = new Set(s.ids); if (ids.has(id)) ids.delete(id); else ids.add(id); return { ids };
  }),
  setOnly: (id) => set({ ids: new Set([id]), allMatching: false, excluded: new Set() }),
  clear: () => set({ ids: new Set(), allMatching: false, excluded: new Set() }),
  selectAll: () => set({ allMatching: true, ids: new Set(), excluded: new Set() }),
  isSelected: (id) => { const s = get(); return s.allMatching ? !s.excluded.has(id) : s.ids.has(id); },
  count: (total) => { const s = get(); return s.allMatching ? Math.max(0, total - s.excluded.size) : s.ids.size; },
}));

export interface Toast { id: number; kind: 'ok' | 'err'; text: string }
interface Toasts { items: Toast[]; push: (kind: 'ok' | 'err', text: string) => void; dismiss: (id: number) => void }
let seq = 0;
export const useToasts = create<Toasts>((set) => ({
  items: [],
  push: (kind, text) => {
    const id = ++seq;
    set((s) => ({ items: [...s.items.slice(-4), { id, kind, text }] }));
    setTimeout(() => set((s) => ({ items: s.items.filter((i) => i.id !== id) })), kind === 'err' ? 7000 : 3500);
  },
  dismiss: (id) => set((s) => ({ items: s.items.filter((i) => i.id !== id) })),
}));

type Theme = 'dark' | 'light';
interface Ui { theme: Theme; setTheme: (t: Theme) => void; refreshTick: number; bump: () => void }
export const useUi = create<Ui>((set) => ({
  theme: (localStorage.getItem('vanta.theme') as Theme) || 'dark',
  setTheme: (theme) => { localStorage.setItem('vanta.theme', theme); document.documentElement.dataset.theme = theme; set({ theme }); },
  refreshTick: 0,
  bump: () => set((s) => ({ refreshTick: s.refreshTick + 1 })),
}));
