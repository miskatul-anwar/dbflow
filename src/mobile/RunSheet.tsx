import type { ProcessingState } from '../services/api/databaseService';
import { PHASES } from './pipeline';
import { PHASE_COLOR } from './PipelineScreen';

/**
 * Progress while the engine runs: percentage, the current step, and which phases have started. A panel above the
 * bottom bar, not a modal: the app bar (workspace switcher) and the bottom bar stay usable during a run.
 */
export function RunSheet({ progress, onCancel }: { progress: ProcessingState | null; onCancel: () => void }) {
  const pct = Math.max(0, Math.min(100, Math.round(progress?.progress ?? 0)));
  const reached = PHASES.findIndex(p => p.id === (progress?.phase ?? 'PREP'));
  return (
    <section className="m-runpanel" aria-label="Running">
      <div className="m-progress-head">
        <span>{PHASES[Math.max(0, reached)]?.title}</span>
        <b>{pct}%</b>
      </div>
      <div className="m-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
        <i style={{ width: `${pct}%` }} />
      </div>
      <p className="m-progress-msg" aria-live="polite" style={{ minHeight: 0 }}>
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
        Cancel run
      </button>
    </section>
  );
}
