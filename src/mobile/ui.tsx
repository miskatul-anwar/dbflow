/** Small building blocks of the phone interface (see mobile.css). */
import { useEffect, type ReactNode } from 'react';
import { motion } from 'framer-motion';
import { ChevronDown, Database, ListChecks, Settings2, Table2 } from 'lucide-react';

export type Screen = 'data' | 'pipeline' | 'results';

/** The workspace's name (tap to switch or manage workspaces) and the screen's state, e.g. "Data · 3 files". */
export function TopBar({
  title,
  detail,
  color,
  running,
  onTitle,
  onSettings,
}: {
  title: string;
  detail: string;
  color: string | null;
  running: boolean;
  onTitle: () => void;
  onSettings: () => void;
}) {
  return (
    <header className="m-top">
      <button className="m-title" onClick={onTitle} aria-label={`Workspace ${title}. Switch or manage workspaces`}>
        <span className="m-ws-dot" style={{ background: color ?? 'var(--m-ink-3)' }} aria-hidden />
        <span className="m-title-text">
          <span className="m-title-name">
            <span>{title}</span>
            <ChevronDown size={16} aria-hidden />
          </span>
          <small>
            {running ? 'Running · ' : ''}
            {detail}
          </small>
        </span>
      </button>
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
    <motion.div className="m-overlay" onClick={onClose} role="presentation" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: 0.18 }}>
      <motion.div
        className="m-sheet"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={e => e.stopPropagation()}
        initial={{ y: 40, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ type: 'spring', stiffness: 420, damping: 36 }}
      >
        <span className="m-grabber" aria-hidden />
        <h2>{title}</h2>
        {children}
      </motion.div>
    </motion.div>
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
    <motion.div
      key={toast.id}
      className={`m-toast${toast.error ? ' m-error' : ''}`}
      role="status"
      onClick={onDone}
      initial={{ opacity: 0, y: 12, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
    >
      {toast.text}
    </motion.div>
  );
}
