import { Check, X } from 'lucide-react';
import type { ProcessingState } from '../services/api/databaseService';
import { PHASES } from './pipeline';
import { PHASE_COLOR } from './PipelineScreen';
import { Sheet } from './ui';

const R = 70;
const C = 2 * Math.PI * R;

/** Progress while the engine runs: a ring, the current message, and the four phases lighting up. */
export function RunSheet({ progress, done, onCancel }: { progress: ProcessingState | null; done: boolean; onCancel: () => void }) {
  const pct = done ? 100 : Math.max(2, Math.min(99, Math.round(progress?.progress ?? 2)));
  const reached = PHASES.findIndex(p => p.id === (progress?.phase ?? 'PREP'));
  return (
    <Sheet label={done ? 'Run finished' : 'Running the pipeline'}>
      {done ? (
        <>
          <div className="m-done">
            <Check size={56} strokeWidth={3} />
          </div>
          <p className="m-run-msg">
            <b>Done!</b> Opening your results…
          </p>
        </>
      ) : (
        <>
          <div className="m-ring" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
            <svg width="168" height="168" viewBox="0 0 168 168" aria-hidden>
              <defs>
                <linearGradient id="m-ring-gradient" x1="0" y1="0" x2="1" y2="1">
                  <stop offset="0%" stopColor="#14b8a6" />
                  <stop offset="100%" stopColor="#8b5cf6" />
                </linearGradient>
              </defs>
              <circle className="m-ring-track" cx="84" cy="84" r={R} />
              <circle className="m-ring-bar" cx="84" cy="84" r={R} strokeDasharray={C} strokeDashoffset={C * (1 - pct / 100)} />
            </svg>
            <div className="m-ring-text">
              <div>
                <b>{pct}%</b>
                <span>{PHASES[Math.max(0, reached)]?.title ?? 'Starting'}</span>
              </div>
            </div>
          </div>
          <p className="m-run-msg" aria-live="polite">
            {progress?.message ?? 'Starting…'}
          </p>
        </>
      )}
      <div className="m-phases" aria-hidden>
        {PHASES.map((p, i) => (
          <div key={p.id} className={done || i <= reached ? 'm-on' : ''} style={{ ['--phase' as string]: PHASE_COLOR[p.id] }}>
            <i />
            {p.title}
          </div>
        ))}
      </div>
      {!done && (
        <button className="m-btn m-btn-soft m-press" onClick={onCancel}>
          <X /> Cancel
        </button>
      )}
    </Sheet>
  );
}
