import { useId, useRef } from 'react';
import { Button } from '@/components/ui/button';
import { Modal } from '@/components/common/Modal';

interface Props {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
  /** `danger` for actions that discard work. */
  tone?: 'default' | 'danger';
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * An in-app question with the same look as the workspace-name dialog (no browser `confirm()`).
 * For destructive actions focus starts on Cancel, so Enter does not discard work by accident.
 */
export function ConfirmDialog({ title, message, confirmLabel, cancelLabel = 'Cancel', tone = 'default', onConfirm, onCancel }: Props) {
  const titleId = useId();
  const messageId = useId();
  const cancelRef = useRef<HTMLButtonElement>(null);
  const danger = tone === 'danger';

  return (
    <Modal
      labelledBy={titleId}
      describedBy={messageId}
      onCancel={onCancel}
      initialFocus={danger ? cancelRef : undefined}
      role={danger ? 'alertdialog' : 'dialog'}
    >
      <h2 id={titleId} className="text-base font-semibold text-gray-900">
        {title}
      </h2>
      <p id={messageId} className="mt-1 text-sm text-gray-600">
        {message}
      </p>
      <div className="mt-5 flex justify-end gap-2">
        <Button ref={cancelRef} type="button" variant="ghost" onClick={onCancel}>
          {cancelLabel}
        </Button>
        <Button type="button" variant={danger ? 'danger' : 'primary'} onClick={onConfirm}>
          {confirmLabel}
        </Button>
      </div>
    </Modal>
  );
}
