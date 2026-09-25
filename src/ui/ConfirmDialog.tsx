import type { ReactNode } from 'react';

import { Button } from './controls';
import { Dialog } from './Dialog';

interface Props {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  /** Styles the confirm button as a destructive action. */
  danger?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Asks before an action that deletes work or produces something partial. */
export function ConfirmDialog({ title, children, confirmLabel, cancelLabel = 'ביטול', danger, busy, onConfirm, onCancel }: Props) {
  return (
    <Dialog
      title={title}
      onClose={onCancel}
      footer={
        <>
          <Button onClick={onCancel} disabled={busy ?? false}>
            {cancelLabel}
          </Button>
          <Button variant="primary" className={danger ? 'btn-danger' : ''} busy={busy ?? false} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="confirm-text">{children}</div>
    </Dialog>
  );
}
