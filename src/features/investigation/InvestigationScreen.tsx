import { useState } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import type { InvestigationNumber } from '../../api/types';
import { formatTimestamp } from '../../lib/format';
import { useResource } from '../../lib/useResource';
import { Banner, Button, Ltr, Spinner } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { CopyButton } from './CopyButton';
import { DistributionDialog } from './DistributionDialog';
import { InvestigationDocument } from './InvestigationDocument';

export function InvestigationScreen({ number, navigation }: { number: InvestigationNumber; navigation: Navigation }) {
  const [investigation] = useResource(() => api.findInvestigation(number));
  const [distributing, setDistributing] = useState(false);

  return (
    <div className="page">
      <div className="page-toolbar">
        <Button variant="subtle" icon="back" onClick={navigation.goHome}>
          דף הבית
        </Button>
        {investigation.status === 'ready' && (
          <Button icon="mail" onClick={() => setDistributing(true)}>
            הפץ במייל
          </Button>
        )}
      </div>

      {investigation.status === 'loading' && <Spinner label="טוען תחקיר" />}
      {investigation.status === 'error' && <Banner tone="error">{errorMessage(investigation.error)}</Banner>}
      {investigation.status === 'ready' && (
        <>
          <div className="storage-info">
            <Icon name="cloud" />
            <span className="storage-label">SharePoint (סימולציה)</span>
            <Ltr className="url">{investigation.data.location}</Ltr>
            <CopyButton value={investigation.data.location} label="העתקה" />
            <span className="storage-meta">
              נוצר {formatTimestamp(investigation.data.createdAt)} · {investigation.data.createdBy}
            </span>
          </div>
          <InvestigationDocument investigation={investigation.data} />
        </>
      )}

      {distributing && <DistributionDialog number={number} onClose={() => setDistributing(false)} />}
    </div>
  );
}
