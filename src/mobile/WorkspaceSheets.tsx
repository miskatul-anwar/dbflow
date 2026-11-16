/** Workspace management on the phone: switch, create, rename, colour and close, as with the desktop tabs. */
import { useState } from 'react';
import { Check, Plus } from 'lucide-react';
import { MAX_TABS, type TabStatus, type WorkspaceTab } from '../hooks/useWorkspaceTabs';
import { TAB_COLOR_PRESETS } from '../utils/tabColors';
import { Sheet } from './ui';

export function WorkspacesSheet({
  tabs,
  activeId,
  status,
  onSelect,
  onNew,
  onEdit,
  onClose,
  onDismiss,
}: {
  tabs: WorkspaceTab[];
  activeId: string;
  status: Record<string, TabStatus>;
  onSelect: (id: string) => void;
  onNew: () => void;
  onEdit: (id: string) => void;
  onClose: (id: string) => void;
  onDismiss: () => void;
}) {
  return (
    <Sheet title="Workspaces" onClose={onDismiss}>
      <div className="m-group">
        {tabs.map(t => {
          const s = status[t.id];
          return (
            <div key={t.id} className="m-row m-ws-row" aria-current={t.id === activeId}>
              <button className="m-row-text" style={{ display: 'flex', alignItems: 'center', gap: 10, minHeight: 48, textAlign: 'left' }} onClick={() => onSelect(t.id)}>
                <span className="m-ws-dot" style={{ background: t.color ?? 'var(--m-ink-3)' }} aria-hidden />
                <span>
                  <strong>{t.title}</strong>
                  <small>{s?.running ? 'Running' : `${s?.operations ?? 0} steps`}</small>
                </span>
              </button>
              <button className="m-btn" onClick={() => onEdit(t.id)}>
                Edit
              </button>
              <button className="m-btn m-danger" onClick={() => onClose(t.id)} aria-label={`Close ${t.title}`}>
                Close
              </button>
            </div>
          );
        })}
      </div>
      <div style={{ height: 12 }} />
      <button className="m-btn m-btn-outline m-btn-block" onClick={onNew} disabled={tabs.length >= MAX_TABS}>
        <Plus /> New workspace
      </button>
      {tabs.length >= MAX_TABS && <p className="m-hint">At most {MAX_TABS} workspaces.</p>}
    </Sheet>
  );
}

/** Name (and, when editing, colour) of a workspace. */
export function WorkspaceForm({
  title,
  initialName,
  initialColor,
  withColor,
  confirmLabel,
  note,
  onConfirm,
  onDismiss,
}: {
  title: string;
  /** A line above the name field, when the sheet needs to say why it asks. */
  note?: string;
  initialName: string;
  initialColor?: string | null;
  withColor: boolean;
  confirmLabel: string;
  onConfirm: (name: string, color: string | null) => void;
  /** Without it the sheet cannot be dismissed: a name must be given (the first workspace, on launch). */
  onDismiss?: () => void;
}) {
  const [name, setName] = useState(initialName);
  const [color, setColor] = useState<string | null>(initialColor ?? null);
  const clean = name.trim().slice(0, 40);
  return (
    <Sheet title={title} onClose={onDismiss}>
      <form
        className="m-stack"
        onSubmit={e => {
          e.preventDefault();
          if (clean) onConfirm(clean, color);
        }}
      >
        {note && <p className="m-hint" style={{ margin: 0 }}>{note}</p>}
        <input className="m-input" value={name} onChange={e => setName(e.target.value)} aria-label="Workspace name" maxLength={40} autoFocus />
        {withColor && (
          <>
            <div className="m-label">Colour</div>
            <div className="m-swatches" role="radiogroup" aria-label="Colour">
              <button type="button" aria-pressed={color === null} aria-label="No colour" onClick={() => setColor(null)} style={{ background: 'var(--m-muted)' }} />
              {TAB_COLOR_PRESETS.map(c => (
                <button
                  key={c.value}
                  type="button"
                  aria-pressed={color === c.value}
                  aria-label={c.name}
                  onClick={() => setColor(c.value)}
                  style={{ background: c.value }}
                />
              ))}
            </div>
          </>
        )}
        <button type="submit" className="m-btn m-btn-primary m-btn-block" disabled={!clean}>
          <Check /> {confirmLabel}
        </button>
      </form>
    </Sheet>
  );
}

/** Confirmation before discarding a workspace's work (or stopping its run). */
export function ConfirmSheet({
  title,
  message,
  confirmLabel,
  onConfirm,
  onDismiss,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onDismiss: () => void;
}) {
  return (
    <Sheet title={title} onClose={onDismiss}>
      <p style={{ marginTop: 0, color: 'var(--m-ink-2)' }}>{message}</p>
      <div className="m-plan-actions">
        <button className="m-btn m-btn-outline" onClick={onDismiss}>
          Keep
        </button>
        <button className="m-btn m-btn-primary" style={{ background: 'var(--m-danger)', color: 'var(--m-on-danger)' }} onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </Sheet>
  );
}
