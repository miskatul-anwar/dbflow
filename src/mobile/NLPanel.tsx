import { useState } from 'react';
import { CornerDownLeft } from 'lucide-react';
import { extensionFor, type NLWorkflow } from '../hooks/useNLWorkflow';
import { dependencyClosure } from '../services/nlp/workflow/planner';

/**
 * The plain-language request box, with the same plan preview as the desktop command bar: the proposed steps
 * (dependencies added automatically are dimmed), then Replace / Extend / Apply & Run, and an undo for the last build.
 */
export function NLPanel({ nl }: { nl: NLWorkflow }) {
  const [draft, setDraft] = useState('');
  const res = nl.resolution;
  const canExtend = res && nl.hasNodes ? extensionFor(nl.placed, res.plan) !== null : false;
  return (
    <>
      <form
        className="m-group m-ask"
        onSubmit={e => {
          e.preventDefault();
          if (draft.trim()) void nl.submit(draft);
        }}
      >
        <input
          value={draft}
          onChange={e => setDraft(e.target.value)}
          placeholder="normalize to BCNF in MySQL"
          aria-label="Describe the pipeline"
          enterKeyHint="go"
        />
        <button type="submit" disabled={!draft.trim() || nl.busy} aria-label="Plan it">
          <CornerDownLeft size={20} />
        </button>
      </form>
      {nl.busy && <p className="m-hint">Planning…</p>}
      {nl.canUndoBuild && !res && !nl.busy && (
        <div className="m-chips">
          <button className="m-chip" onClick={nl.undoBuild}>
            Undo last build
          </button>
        </div>
      )}

      {res && !nl.busy && (
        <section className="m-group m-plan" aria-label="Proposed pipeline" style={{ marginTop: 8 }}>
          {res.plan.length > 0 ? (
            <>
              <div className="m-row-text">
                <strong>
                  {res.plan.length} {res.plan.length === 1 ? 'step' : 'steps'}
                  {res.confidence === 'medium' ? ' (best guess)' : ''}
                </strong>
                <small>“{nl.query}”</small>
              </div>
              <ol aria-label="Proposed steps">
                {res.plan.map(op => {
                  const dep = res.addedAsDependencies.includes(op);
                  const neededBy = dep ? res.targets.find(t => dependencyClosure([t]).has(op)) : null;
                  return (
                    <li key={op} className={dep ? 'm-dep' : ''} title={dep ? `Needed by ${neededBy ?? 'a later step'}` : 'Requested'}>
                      {op}
                    </li>
                  );
                })}
              </ol>
              {res.message && <p className="m-warn">{res.message}</p>}
              {res.options.dialect && <p className="m-hint" style={{ margin: '0 0 8px' }}>SQL dialect: {res.options.dialect}</p>}
              <div className="m-plan-actions">
                {nl.hasNodes ? (
                  <>
                    <button className="m-btn m-btn-outline" onClick={() => nl.apply(res, { run: false, mode: 'replace' })}>
                      Replace
                    </button>
                    <button className="m-btn m-btn-outline" disabled={!canExtend} onClick={() => nl.apply(res, { run: false, mode: 'extend' })}>
                      Extend
                    </button>
                  </>
                ) : (
                  <button className="m-btn m-btn-outline" onClick={() => nl.apply(res, { run: false })}>
                    Apply
                  </button>
                )}
                <button
                  className="m-btn m-btn-primary"
                  disabled={!nl.hasFiles}
                  onClick={() => nl.apply(res, { run: true, mode: nl.hasNodes && canExtend ? 'extend' : 'replace' })}
                >
                  Apply &amp; run
                </button>
                <button className="m-btn" onClick={nl.dismiss}>
                  Dismiss
                </button>
              </div>
              {!nl.hasFiles && <p className="m-hint">Add files to run.</p>}
            </>
          ) : (
            <>
              <p className="m-warn" style={{ marginTop: 0 }}>
                {res.message ?? 'No operation matched that request.'}
              </p>
              {res.candidates.length > 0 && (
                <div className="m-chips" style={{ margin: '8px 0' }}>
                  {res.candidates.slice(0, 4).map(c => (
                    <button key={c.operation} className="m-chip" onClick={() => void nl.pickCandidate(c.operation)}>
                      {c.operation}
                    </button>
                  ))}
                </div>
              )}
              <button className="m-btn" onClick={nl.dismiss}>
                Dismiss
              </button>
            </>
          )}
        </section>
      )}
    </>
  );
}
