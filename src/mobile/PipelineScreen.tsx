import { useRef, useState } from 'react';
import { Check, FolderOpen, Redo2, Save, Undo2 } from 'lucide-react';
import { OPERATIONS } from '../config/operations';
import type { NLWorkflow } from '../hooks/useNLWorkflow';
import { GOALS, PHASES, goalOf, planForGoal, phaseOf, type Phase } from './pipeline';
import { NLPanel } from './NLPanel';
import { isTauriMobile } from '../utils/platform';

export const PHASE_COLOR: Record<Phase, string> = {
  PREP: 'var(--m-prep)',
  ANALYZE: 'var(--m-analyze)',
  NORMAL: 'var(--m-normal)',
  OUTPUT: 'var(--m-output)',
};

export interface PipelineControls {
  /** Placed operations, in run order. */
  steps: string[];
  canUndo: boolean;
  canRedo: boolean;
  undo: () => void;
  redo: () => void;
  /** Replace the pipeline with these operations. */
  setSteps: (steps: string[]) => void;
  /** Add the operation with its prerequisites, or remove it with the steps that need it. */
  toggle: (op: string) => void;
  save: () => void;
  open: (file: File) => void;
}

export function PipelineScreen({ pipeline, nl }: { pipeline: PipelineControls; nl: NLWorkflow }) {
  const [filter, setFilter] = useState('');
  const fileInput = useRef<HTMLInputElement>(null);
  const { steps } = pipeline;
  const current = goalOf(steps);
  const all = Object.keys(OPERATIONS);
  const q = filter.trim().toLowerCase();
  const matches = (op: string) => !q || op.toLowerCase().includes(q) || OPERATIONS[op].description.toLowerCase().includes(q);

  return (
    <>
      <div className="m-group m-toolbar" role="toolbar" aria-label="Pipeline">
        <button onClick={pipeline.undo} disabled={!pipeline.canUndo}>
          <Undo2 size={20} /> Undo
        </button>
        <button onClick={pipeline.redo} disabled={!pipeline.canRedo}>
          <Redo2 size={20} /> Redo
        </button>
        <button onClick={() => fileInput.current?.click()}>
          <FolderOpen size={20} /> Open
        </button>
        <button onClick={pipeline.save} disabled={steps.length === 0}>
          <Save size={20} /> Save
        </button>
      </div>
      <input
        ref={fileInput}
        type="file"
        hidden
        accept={isTauriMobile() ? undefined : '.json'}
        onChange={e => {
          const f = e.target.files?.[0];
          e.target.value = '';
          if (f) pipeline.open(f);
        }}
      />

      <div className="m-label">Goal</div>
      <div className="m-group" role="radiogroup" aria-label="Goal">
        {GOALS.map(g => {
          const plan = planForGoal(g);
          return (
            <button key={g.id} className="m-row" role="radio" aria-checked={current?.id === g.id} onClick={() => pipeline.setSteps(plan)}>
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
      <NLPanel nl={nl} />

      <div className="m-label">
        Steps · {steps.length} of {all.length}
      </div>
      <input className="m-input" type="search" placeholder="Filter steps" value={filter} onChange={e => setFilter(e.target.value)} aria-label="Filter steps" />
      <div style={{ height: 8 }} />
      {PHASES.map(phase => {
        const ops = all.filter(o => phaseOf(o) === phase.id && matches(o));
        if (!ops.length) return null;
        const inPhase = all.filter(o => phaseOf(o) === phase.id);
        return (
          <section key={phase.id} className="m-group" style={{ ['--phase' as string]: PHASE_COLOR[phase.id] }} aria-label={phase.title}>
            <div className="m-phase-head">
              <span className="m-swatch" aria-hidden />
              <span>{phase.title}</span>
              <span>
                {inPhase.filter(o => steps.includes(o)).length}/{inPhase.length}
              </span>
            </div>
            {ops.map(op => {
              const missing = OPERATIONS[op].dependencies.filter(d => !steps.includes(d));
              return (
                <button key={op} className="m-row" role="checkbox" aria-checked={steps.includes(op)} onClick={() => pipeline.toggle(op)}>
                  <span className="m-box" aria-hidden>
                    <Check size={16} strokeWidth={3} />
                  </span>
                  <span className="m-row-text">
                    <strong>{op}</strong>
                    <small>
                      {OPERATIONS[op].description}
                      {!steps.includes(op) && missing.length > 0 ? ` · adds ${missing.join(', ')}` : ''}
                    </small>
                  </span>
                </button>
              );
            })}
          </section>
        );
      })}
      {q && !all.some(matches) && <p className="m-hint">No step matches “{filter}”.</p>}
    </>
  );
}
