/** Small building blocks of the phone interface (see mobile.css). */
import { useEffect, type ReactNode } from 'react';
import { Database, ListChecks, Settings2, Table2 } from 'lucide-react';

export type Screen = 'data' | 'pipeline' | 'results';

/** The screen's name and its current state, e.g. "Data · 3 files, 19 KB". */
export function TopBar({ title, detail, onSettings }: { title: string; detail: string; onSettings: () => void }) {
  return (
    <header className="m-top">
      <div>
        <h1>{title}</h1>
        <p>{detail}</p>
      </div>
      <button className="m-icon-btn" onClick={onSettings} aria-label="Settings">
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
  const item = (id: Screen, label: string, icon: ReactNode, count?: ReactNode) => (
    <button aria-current={screen === id ? 'page' : undefined} onClick={() => onGo(id)}>
      {icon}
      {label}
      {count}
    </button>
  );
  return (
    <nav className="m-nav" aria-label="Screens">
      {item('data', 'Data', <Database />, files > 0 ? <span className="m-count">{files}</span> : null)}
      {item('pipeline', 'Pipeline', <ListChecks />)}
      {item('results', 'Results', <Table2 />, resultsReady && screen !== 'results' ? <span className="m-count">new</span> : null)}
    </nav>
  );
}

/** A sheet from the bottom of the screen. Tapping outside closes it (when `onClose` is given). */
export function Sheet({ title, onClose, children }: { title: string; onClose?: () => void; children: ReactNode }) {
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);
  return (
    <div className="m-overlay" onClick={onClose} role="presentation">
      <div className="m-sheet" role="dialog" aria-modal="true" aria-label={title} onClick={e => e.stopPropagation()}>
        <h2>{title}</h2>
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
    const t = setTimeout(onDone, toast.error ? 6000 : 3000);
    return () => clearTimeout(t);
  }, [toast, onDone]);
  if (!toast) return null;
  return (
    <div className={`m-toast${toast.error ? ' m-error' : ''}`} role="status" onClick={onDone}>
      {toast.text}
    </div>
  );
}
