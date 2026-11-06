import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import './styles.css';
import { useUi } from './store';
import { useI18n } from './i18n';

document.documentElement.dataset.theme = useUi.getState().theme;
document.documentElement.lang = useI18n.getState().lang;
// Block the WebView's context menu and file drops (no accidental navigation away from the app).
window.addEventListener('contextmenu', (e) => { if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) e.preventDefault(); });
window.addEventListener('dragover', (e) => e.preventDefault());
window.addEventListener('drop', (e) => e.preventDefault());

createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>);
