import { useState } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import type { InvestigationNumber } from '../../api/types';
import { formatTimestamp } from '../../lib/format';
import { INVESTIGATION_STATUS_LABELS } from '../../lib/labels';
import { useResource } from '../../lib/useResource';
import { Banner, Button, Ltr, Spinner } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { CopyButton } from './CopyButton';
import { DistributionDialog } from './DistributionDialog';
import { DocumentViewer } from './DocumentViewer';
import { ExportPdfButton, type Notice } from './ExportPdfButton';

export function InvestigationScreen({ number, navigation }: { number: InvestigationNumber; navigation: Navigation }) {
  const [details, reload] = useResource(() => api.getInvestigation(number));
  const [distributing, setDistributing] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);

  return (
    <div className="page">
      <div className="page-toolbar">
        <Button variant="subtle" icon="back" onClick={navigation.goHome}>
          דף הבית
        </Button>
        {details.status === 'ready' && (
          <div className="page-toolbar-actions">
            <ExportPdfButton target={{ kind: 'investigation', number }} onNotice={setNotice} />
            <Button variant="primary" icon="mail" onClick={() => setDistributing(true)}>
              {details.data.investigation.lifecycle.status === 'distributed' ? 'הפצה חוזרת' : 'הפצה במייל'}
            </Button>
          </div>
        )}
      </div>

      {notice && <Banner tone={notice.tone}>{notice.content}</Banner>}
      {details.status === 'loading' && <Spinner label="טוען תחקיר" />}
      {details.status === 'error' && <Banner tone="error">{errorMessage(details.error)}</Banner>}
      {details.status === 'ready' && (
        <>
          <div className="storage-info">
            <Icon name="cloud" />
            <span className="storage-label">
              {details.data.investigation.publication.destination === 'sharePoint'
                ? 'פריט SharePoint (סימולציה)'
                : 'תיקייה משותפת'}
            </span>
            <Ltr className="url">{details.data.investigation.publication.url}</Ltr>
            <CopyButton value={details.data.investigation.publication.url} label="העתקה" />
            <ul className="lifecycle" aria-label="היסטוריית סטטוס">
              {details.data.investigation.lifecycle.history.map((event, index) => (
                <li key={index}>
                  <strong>{INVESTIGATION_STATUS_LABELS[event.status]}</strong> {formatTimestamp(event.at)} ·{' '}
                  <bdi>{event.by}</bdi>
                </li>
              ))}
            </ul>
          </div>
          <DocumentViewer document={details.data.document} />
        </>
      )}

      {distributing && (
        <DistributionDialog
          number={number}
          onClose={() => {
            setDistributing(false);
            reload();
          }}
        />
      )}
    </div>
  );
}
