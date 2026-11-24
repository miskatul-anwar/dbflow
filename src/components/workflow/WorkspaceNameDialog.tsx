import { useId, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Modal } from '@/components/common/Modal';

export const MAX_WORKSPACE_NAME = 40;

interface Props {
  /** "Name your workspace" on first launch, "New workspace" for the + button. */
  title: string;
  /** What the field starts with; typing replaces it. */
  suggestion: string;
  /** Names already in use, so the dialog can say so (it still allows them). */
  existing?: string[];
  confirmLabel?: string;
  /** Shown above the field, e.g. what closing the last workspace discards. */
  warning?: string;
  /** Omit on first launch, where a workspace must exist. */
  onCancel?: () => void;
  onConfirm: (name: string) => void;
}

/**
 * Asks for a workspace name. Focus starts in the field with the suggestion selected, Enter creates and Escape
 * cancels (where cancelling makes sense). Shares its look with every other dialog through `Modal`.
 */
export function WorkspaceNameDialog({ title, suggestion, existing = [], confirmLabel = 'Create workspace', warning, onCancel, onConfirm }: Props) {
  const [name, setName] = useState(suggestion);
  const inputRef = useRef<HTMLInputElement>(null);
  const titleId = useId();

  const clean = name.trim().slice(0, MAX_WORKSPACE_NAME);
  const taken = existing.some(e => e.toLowerCase() === clean.toLowerCase());
  const submit = () => {
    if (clean) onConfirm(clean);
  };

  return (
    <Modal labelledBy={titleId} onCancel={onCancel} initialFocus={inputRef}>
      <form
        onSubmit={e => {
          e.preventDefault();
          submit();
        }}
      >
        <h2 id={titleId} className="text-base font-semibold text-gray-900">
          {title}
        </h2>
        {warning && <p className="mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900">{warning}</p>}
        <label className="mt-4 block text-xs font-medium text-gray-600" htmlFor={`${titleId}-name`}>
          Workspace name
        </label>
        <input
          id={`${titleId}-name`}
          ref={inputRef}
          value={name}
          maxLength={MAX_WORKSPACE_NAME}
          onChange={e => setName(e.target.value)}
          autoComplete="off"
          spellCheck={false}
          className="mt-1 h-9 w-full rounded-lg border border-gray-300 bg-white px-3 text-sm text-gray-900 focus:border-teal-600 focus:outline-none focus:ring-2 focus:ring-teal-600/30"
        />
        <p className="mt-1.5 min-h-4 text-xs text-gray-600" aria-live="polite">
          {!clean ? 'Give it a name to continue.' : taken ? 'Another workspace already has this name. You can still use it.' : ''}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          {onCancel && (
            <Button type="button" variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
          )}
          <Button type="submit" variant="primary" disabled={!clean}>
            {confirmLabel}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
