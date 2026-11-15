/** Small building blocks of the phone interface (see mobile.css). */
import { useEffect, type ReactNode } from 'react';
import { Database, Settings2, Sparkles, Workflow } from 'lucide-react';

export type Screen = 'data' | 'pipeline' | 'results';

export function TopBar({ subtitle, onSettings }: { subtitle: string; onSettings: () => void }) {
  return (
    <header className="m-top">
      <div className="m-brand">
        <img src="/icon.png" alt="" />
        <div>
          DBFlow
          <small>{subtitle}</small>
        </div>
      </div>
      <button className="m-icon-btn m-press" onClick={onSettings} aria-label="Settings">
        <Settings2 size={22} />
      </button>
    </header>
  );
}

export function BottomNav({
  screen,
  onGo,
  files,
  resultsReady,
}: {
  screen: Screen;
  onGo: (s: Screen) => void;
  files: number;
  resultsReady: boolean;
}) {
  const item = (id: Screen, label: string, icon: ReactNode, extra?: ReactNode) => (
    <button className="m-press" aria-current={screen === id ? 'page' : undefined} onClick={() => onGo(id)}>
      {icon}
      {label}
      {extra}
    </button>
  );
  return (
    <nav className="m-nav" aria-label="Screens">
      {item('data', 'Data', <Database />, files > 0 ? <span className="m-badge">{files}</span> : null)}
      {item('pipeline', 'Pipeline', <Workflow />)}
      {item('results', 'Results', <Sparkles />, resultsReady && screen !== 'results' ? <span className="m-dot" aria-label="new results" /> : null)}
    </nav>
  );
}

/** A sheet that rises from the bottom. Tapping outside closes it (when `onClose` is given). */
export function Sheet({ label, onClose, children }: { label: string; onClose?: () => void; children: ReactNode }) {
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);
  return (
    <div className="m-overlay" onClick={onClose} role="presentation">
      <div className="m-sheet" role="dialog" aria-modal="true" aria-label={label} onClick={e => e.stopPropagation()}>
        <div className="m-grip" />
        {children}
      </div>
    </div>
  );
}

export function Switch({ on, onChange, label }: { on: boolean; onChange: (on: boolean) => void; label: string }) {
  return <span className="m-switch" role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />;
}

export interface ToastMessage {
  id: number;
  text: string;
  error?: boolean;
}

export function Toast({ toast, onDone }: { toast: ToastMessage | null; onDone: () => void }) {
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(onDone, toast.error ? 6000 : 3200);
    return () => clearTimeout(t);
  }, [toast, onDone]);
  if (!toast) return null;
  return (
    <div className={`m-toast${toast.error ? ' m-error' : ''}`} role="status" onClick={onDone}>
      {toast.text}
    </div>
  );
}
