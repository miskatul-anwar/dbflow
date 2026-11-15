import { useState } from 'react';
import { Check, Send, Sparkles } from 'lucide-react';
import { OPERATIONS } from '../config/operations';
import { GOALS, PHASES, goalOf, planForGoal, phaseOf, type Phase } from './pipeline';

export const PHASE_COLOR: Record<Phase, string> = {
  PREP: 'var(--m-prep)',
  ANALYZE: 'var(--m-analyze)',
  NORMAL: 'var(--m-normal)',
  OUTPUT: 'var(--m-output)',
};

const GOAL_COLOR: Record<string, string> = {
  '3nf': 'var(--m-normal)',
  bcnf: '#6d28d9',
  keys: 'var(--m-analyze)',
  profile: 'var(--m-prep)',
  all: 'var(--m-output)',
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
    <div className="m-enter">
      <h1 className="m-h1">What should we build?</h1>
      <p className="m-lead">Pick a goal, or just say it. You can switch single steps on and off below.</p>

      <div className="m-goals" role="group" aria-label="Goals">
        {GOALS.map(g => {
          const selected = current?.id === g.id;
          return (
            <button
              key={g.id}
              className="m-goal m-press"
              aria-pressed={selected}
              style={{ ['--goal-color' as string]: GOAL_COLOR[g.id] }}
              onClick={() => onSteps(planForGoal(g))}
            >
              <strong>{g.title}</strong>
              <p>{g.blurb}</p>
              <span className="m-goal-foot">
                {planForGoal(g).length} steps
                {selected && (
                  <span className="m-check">
                    <Check size={16} strokeWidth={3} />
                  </span>
                )}
              </span>
            </button>
          );
        })}
      </div>

      <form
        className="m-card m-ask"
        onSubmit={e => {
          e.preventDefault();
          if (text.trim()) onAsk(text);
        }}
      >
        <Sparkles size={20} color="var(--m-accent)" aria-hidden />
        <input
          value={text}
          onChange={e => setText(e.target.value)}
          placeholder="e.g. normalize to BCNF in MySQL"
          aria-label="Describe the pipeline"
          enterKeyHint="go"
        />
        <button className="m-send m-press" type="submit" disabled={!text.trim() || ask.busy} aria-label="Plan it">
          <Send size={20} />
        </button>
      </form>
      {ask.busy && <p className="m-note">Reading your request…</p>}
      {!ask.busy && ask.message && <p className="m-note">{ask.message}</p>}
      {!ask.busy && ask.candidates.length > 0 && (
        <div className="m-chips">
          {ask.candidates.map(c => (
            <button key={c} className="m-chip m-press" onClick={() => onPickCandidate(c)}>
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
        const on = ops.filter(o => steps.includes(o)).length;
        return (
          <section key={phase.id} className="m-card m-phase" style={{ ['--phase' as string]: PHASE_COLOR[phase.id] }}>
            <div className="m-phase-head">
              <span className="m-phase-dot" />
              <span>{phase.title}</span>
              <span>
                {on}/{ops.length}
              </span>
            </div>
            {ops.map(op => {
              const active = steps.includes(op);
              return (
                <button key={op} className="m-step m-press" aria-pressed={active} onClick={() => onToggle(op)}>
                  <span className="m-step-toggle">
                    <Check size={18} strokeWidth={3} />
                  </span>
                  <span>
                    <strong>{op}</strong>
                    <small>{OPERATIONS[op].description}</small>
                  </span>
                </button>
              );
            })}
          </section>
        );
      })}
    </div>
  );
}
