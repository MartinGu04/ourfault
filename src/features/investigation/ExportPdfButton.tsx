// PDF export. A completed or distributed investigation exports directly. A
// draft first asks for confirmation, and the backend marks every page of
// the file as a draft.

import { useState, type ReactNode } from 'react';

import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import type { InvestigationNumber } from '../../api/types';
import { Button, Ltr } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/ConfirmDialog';

export type ExportTarget =
  | { kind: 'investigation'; number: InvestigationNumber }
  /** `prepare` saves the draft and returns its id (null if it cannot). */
  | { kind: 'draft'; prepare: () => Promise<string | null> };

export interface Notice {
  tone: 'success' | 'error';
  content: ReactNode;
}

interface Props {
  target: ExportTarget;
  onNotice: (notice: Notice) => void;
  label?: string;
  size?: 'normal' | 'large';
}

export function ExportPdfButton({ target, onNotice, label = 'ייצוא ל-PDF', size = 'normal' }: Props) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  async function run(confirmed: boolean) {
    setBusy(true);
    try {
      let file;
      if (target.kind === 'investigation') {
        file = await api.exportInvestigationPdf(target.number);
      } else {
        const id = await target.prepare();
        if (!id) {
          onNotice({ tone: 'error', content: 'הטיוטה עדיין לא נשמרה. מלאו פרט כלשהו ונסו שוב.' });
          return;
        }
        file = await api.exportDraftPdf(id, confirmed);
      }
      onNotice({
        tone: 'success',
        content: (
          <>
            הקובץ <Ltr>{file.fileName}</Ltr> נשמר בתיקייה <Ltr className="url">{file.folder}</Ltr>
          </>
        ),
      });
    } catch (caught) {
      onNotice({ tone: 'error', content: errorMessage(caught) });
    } finally {
      setBusy(false);
      setConfirming(false);
    }
  }

  return (
    <>
      <Button icon="download" size={size} busy={busy && !confirming} onClick={() => (target.kind === 'draft' ? setConfirming(true) : run(false))}>
        {label}
      </Button>
      {confirming && (
        <ConfirmDialog
          title="ייצוא טיוטה"
          confirmLabel="ייצוא כטיוטה"
          busy={busy}
          onConfirm={() => run(true)}
          onCancel={() => setConfirming(false)}
        >
          <p>התחקיר עדיין לא הושלם או הופץ. הקובץ שייווצר עלול להיות חלקי. האם להמשיך?</p>
          <p className="confirm-note">כל עמוד בקובץ יסומן „טיוטה — לא להפצה”.</p>
        </ConfirmDialog>
      )}
    </>
  );
}
