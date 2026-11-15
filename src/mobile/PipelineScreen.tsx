import { useState } from 'react';
import { Check, CornerDownLeft } from 'lucide-react';
import { OPERATIONS } from '../config/operations';
import { GOALS, PHASES, goalOf, planForGoal, phaseOf, type Phase } from './pipeline';

export const PHASE_COLOR: Record<Phase, string> = {
  PREP: 'var(--m-prep)',
  ANALYZE: 'var(--m-analyze)',
  NORMAL: 'var(--m-normal)',
  OUTPUT: 'var(--m-output)',
};

export interface AskState {
  busy: boolean;
  message: string | null;
  candidates: string[];
}

export function PipelineScreen({
  steps,
  onSteps,
  onToggle,
  ask,
  onAsk,
  onPickCandidate,
}: {
  steps: string[];
  onSteps: (steps: string[]) => void;
  onToggle: (op: string) => void;
  ask: AskState;
  onAsk: (text: string) => void;
  onPickCandidate: (op: string) => void;
}) {
  const [text, setText] = useState('');
  const current = goalOf(steps);
  const all = Object.keys(OPERATIONS);
  return (
    <>
      <div className="m-label">Goal</div>
      <div className="m-group" role="radiogroup" aria-label="Goal">
        {GOALS.map(g => {
          const plan = planForGoal(g);
          return (
            <button key={g.id} className="m-row" role="radio" aria-checked={current?.id === g.id} onClick={() => onSteps(plan)}>
              <span className="m-radio" aria-hidden />
              <span className="m-row-text">
                <strong>{g.title}</strong>
                <small>{g.blurb}</small>
              </span>
              <span className="m-row-end">{plan.length} steps</span>
            </button>
          );
        })}
      </div>
      {!current && steps.length > 0 && <p className="m-hint">Custom selection: {steps.length} steps.</p>}

      <div className="m-label">Or describe it</div>
      <form
        className="m-group m-ask"
        onSubmit={e => {
          e.preventDefault();
          if (text.trim()) onAsk(text);
        }}
      >
        <input
          value={text}
          onChange={e => setText(e.target.value)}
          placeholder="normalize to BCNF in MySQL"
          aria-label="Describe the pipeline"
          enterKeyHint="go"
        />
        <button type="submit" disabled={!text.trim() || ask.busy} aria-label="Plan it">
          <CornerDownLeft size={20} />
        </button>
      </form>
      {ask.busy && <p className="m-hint">Planning…</p>}
      {!ask.busy && ask.message && <p className="m-hint">{ask.message}</p>}
      {!ask.busy && ask.candidates.length > 0 && (
        <div className="m-chips">
          {ask.candidates.map(c => (
            <button key={c} className="m-chip" onClick={() => onPickCandidate(c)}>
              {c}
            </button>
          ))}
        </div>
      )}

      <div className="m-label">
        Steps · {steps.length} of {all.length}
      </div>
      {PHASES.map(phase => {
        const ops = all.filter(o => phaseOf(o) === phase.id);
        return (
          <section key={phase.id} className="m-group" style={{ ['--phase' as string]: PHASE_COLOR[phase.id] }} aria-label={phase.title}>
            <div className="m-phase-head">
              <span className="m-swatch" aria-hidden />
              <span>{phase.title}</span>
              <span>
                {ops.filter(o => steps.includes(o)).length}/{ops.length}
              </span>
            </div>
            {ops.map(op => (
              <button key={op} className="m-row" role="checkbox" aria-checked={steps.includes(op)} onClick={() => onToggle(op)}>
                <span className="m-box" aria-hidden>
                  <Check size={16} strokeWidth={3} />
                </span>
                <span className="m-row-text">
                  <strong>{op}</strong>
                  <small>{OPERATIONS[op].description}</small>
                </span>
              </button>
            ))}
          </section>
        );
      })}
    </>
  );
}
