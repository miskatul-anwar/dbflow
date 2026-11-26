import { useEffect, useRef, useState } from 'react';
import { toast as sonner } from 'sonner';
import { motion } from 'framer-motion';
import { Check, Loader2 } from 'lucide-react';
import type { ProcessingState } from '../services/api/databaseService';
import { PHASES } from './pipeline';
import { PHASE_COLOR } from './PipelineScreen';

const VERB: Record<string, string> = { PREP: 'Preparing', ANALYZE: 'Analyzing', NORMAL: 'Normalizing', OUTPUT: 'Generating' };

/** "m:ss" since the run started. */
export function formatElapsed(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/** The engine reports "<table>: <what it is doing>"; the action reads first, the table under it. */
export function splitMessage(message: string | undefined): { action: string; table?: string } {
  const m = (message ?? '').trim();
  if (!m) return { action: 'Starting…' };
  const i = m.indexOf(': ');
  if (i <= 0) return { action: m };
  const action = m.slice(i + 2);
  return { action: action.charAt(0).toUpperCase() + action.slice(1), table: m.slice(0, i) };
}

function useElapsed(): number {
  const [start] = useState(() => Date.now());
  const [now, setNow] = useState(start);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  return (now - start) / 1000;
}

/**
 * Progress while the engine runs: a floating card above the bottom bar (not a modal, so the app bar and the bottom
 * bar stay usable). The phase verb and a spinner, percent and elapsed time, a bar in the current phase's colour, what
 * the engine is doing to which table, and a four-step tracker of the phases.
 */
export function RunSheet({ progress, onCancel }: { progress: ProcessingState | null; onCancel: () => void }) {
  const pct = Math.max(0, Math.min(100, Math.round(progress?.progress ?? 0)));
  const reached = Math.max(0, PHASES.findIndex(p => p.id === (progress?.phase ?? 'PREP')));
  const phase = PHASES[reached];
  const elapsed = useElapsed();
  const { action, table } = splitMessage(progress?.message);
  const card = useRef<HTMLElement>(null);

  // Toasts left from before the run are dismissed (the user has moved on), and new ones stack above this card.
  useEffect(() => {
    sonner.dismiss();
    const root = document.documentElement;
    const lift = () => {
      const h = card.current?.getBoundingClientRect().height ?? 0;
      root.style.setProperty('--nf-toast-lift', `${Math.round(h + 96)}px`);
    };
    lift();
    const ro = typeof ResizeObserver !== 'undefined' && card.current ? new ResizeObserver(lift) : null;
    if (ro && card.current) ro.observe(card.current);
    return () => {
      ro?.disconnect();
      root.style.removeProperty('--nf-toast-lift');
    };
  }, []);

  return (
    <motion.section
      ref={card}
      className="m-runpanel"
      aria-label="Running"
      style={{ ['--phase' as string]: PHASE_COLOR[phase.id] }}
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
    >
      <div className="m-run-head">
        <span className="m-run-spin" aria-hidden>
          <Loader2 size={18} />
        </span>
        <div className="m-run-title">
          <strong>{VERB[phase.id]}</strong>
          <span>
            <b>{pct}%</b> · {formatElapsed(elapsed)}
          </span>
        </div>
        <button className="m-run-cancel" onClick={onCancel}>
          Cancel
        </button>
      </div>

      <div className="m-run-bar" role="progressbar" aria-label="Progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
        <motion.i initial={false} animate={{ width: `${pct}%` }} transition={{ duration: 0.35, ease: [0.16, 1, 0.3, 1] }} />
      </div>

      <p className="m-run-msg" aria-live="polite">
        <span>{action}</span>
        {table && <small>{table}</small>}
      </p>

      <ol className="m-run-steps" aria-label="Phases">
        {PHASES.map((p, i) => {
          const state = i < reached ? 'done' : i === reached ? 'current' : 'todo';
          return (
            <li key={p.id} data-state={state} style={{ ['--phase' as string]: PHASE_COLOR[p.id] }} aria-current={state === 'current' ? 'step' : undefined}>
              <span className="m-run-dot" aria-hidden>
                {state === 'done' && <Check size={10} strokeWidth={3.5} />}
              </span>
              {p.title}
            </li>
          );
        })}
      </ol>
    </motion.section>
  );
}
