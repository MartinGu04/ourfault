import { useState } from 'react';

import type { Navigation } from '../../App';
import type { InvestigationNumber, StoredInvestigation } from '../../api/types';
import { rowsLabel } from '../../lib/format';
import { Banner, Button, Ltr } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { DistributionDialog } from '../investigation/DistributionDialog';

interface Props {
  created: StoredInvestigation;
  previewNumber: InvestigationNumber | null;
  navigation: Navigation;
}

export function SuccessStep({ created, previewNumber, navigation }: Props) {
  const [distributing, setDistributing] = useState(false);
  const numberChanged = previewNumber !== null && previewNumber !== created.number;

  return (
    <div className="success">
      <div className="success-icon">
        <Icon name="check" size={32} />
      </div>
      <h2 className="success-title">התחקיר נוצר בהצלחה</h2>
      <p className="success-number">
        <Ltr>{created.number}</Ltr>
      </p>
      <p className="success-summary">
        {created.system.name} · {created.template.name} · {rowsLabel(created.rows.length)}
      </p>

      {numberChanged && (
        <Banner>
          המספר <Ltr>{previewNumber}</Ltr> נתפס בינתיים בתחקיר אחר, ולכן התחקיר נשמר במספר <Ltr>{created.number}</Ltr>.
        </Banner>
      )}

      <div className="success-location">
        <Icon name="cloud" />
        <div>
          <span className="success-location-label">נשמר ב-SharePoint (סימולציה)</span>
          <Ltr className="url">{created.location}</Ltr>
        </div>
      </div>

      <div className="success-actions">
        <Button variant="primary" size="large" onClick={() => navigation.openInvestigation(created.number)}>
          פתח תחקיר
        </Button>
        <Button size="large" icon="mail" onClick={() => setDistributing(true)}>
          הפץ במייל
        </Button>
      </div>
      <button type="button" className="link-button" onClick={navigation.goHome}>
        חזרה לדף הבית
      </button>

      {distributing && <DistributionDialog number={created.number} onClose={() => setDistributing(false)} />}
    </div>
  );
}
