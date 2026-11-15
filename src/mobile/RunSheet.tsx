import type { ProcessingState } from '../services/api/databaseService';
import { PHASES } from './pipeline';
import { PHASE_COLOR } from './PipelineScreen';
import { Sheet } from './ui';

/** Progress while the engine runs: percentage, the current step, and which phases have started. */
export function RunSheet({ progress, onCancel }: { progress: ProcessingState | null; onCancel: () => void }) {
  const pct = Math.max(0, Math.min(100, Math.round(progress?.progress ?? 0)));
  const reached = PHASES.findIndex(p => p.id === (progress?.phase ?? 'PREP'));
  return (
    <Sheet title="Running">
      <div className="m-progress-head">
        <span>{PHASES[Math.max(0, reached)]?.title}</span>
        <b>{pct}%</b>
      </div>
      <div className="m-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
        <i style={{ width: `${pct}%` }} />
      </div>
      <p className="m-progress-msg" aria-live="polite">
        {progress?.message ?? 'Starting…'}
      </p>
      <div className="m-phase-list" aria-hidden>
        {PHASES.map((p, i) => (
          <span key={p.id} className={i <= reached ? 'm-on' : ''} style={{ ['--phase' as string]: PHASE_COLOR[p.id] }}>
            {p.title}
          </span>
        ))}
      </div>
      <button className="m-btn m-btn-outline m-btn-block" onClick={onCancel}>
        Cancel
      </button>
    </Sheet>
  );
}
