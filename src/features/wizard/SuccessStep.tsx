import { useState } from 'react';

import type { Navigation } from '../../App';
import type { InvestigationNumber, PublishedInvestigation } from '../../api/types';
import { INVESTIGATION_STATUS_LABELS } from '../../lib/labels';
import { Banner, Button, Ltr } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { CopyButton } from '../investigation/CopyButton';
import { DistributionDialog } from '../investigation/DistributionDialog';
import { ExportPdfButton, type Notice } from '../investigation/ExportPdfButton';

interface Props {
  created: PublishedInvestigation;
  /** The draft had already been published (e.g. a retry); nothing new was created. */
  alreadyExisted: boolean;
  expectedNumber: InvestigationNumber | null;
  navigation: Navigation;
}

export function SuccessStep({ created, alreadyExisted, expectedNumber, navigation }: Props) {
  const [distributing, setDistributing] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const number = created.investigation.number;
  const numberChanged = !alreadyExisted && expectedNumber !== null && expectedNumber !== number;
  const { activity } = created.investigation;

  return (
    <div className="success">
      <div className="success-icon">
        <Icon name="check" size={30} />
      </div>
      <h2 className="success-title">{alreadyExisted ? 'התחקיר כבר נוצר מטיוטה זו' : 'התחקיר נוצר בהצלחה'}</h2>
      <p className="success-number">
        <span className="success-number-label">מספר תחקיר</span>
        <Ltr>{number}</Ltr>
      </p>
      <p className="success-activity">{activity.name}</p>
      <ul className="success-systems" aria-label="מערכות">
        {activity.systems.map((system) => (
          <li key={system.id}>{system.name}</li>
        ))}
      </ul>

      {alreadyExisted && (
        <Banner>
          טיוטה זו כבר הפכה לתחקיר <Ltr>{number}</Ltr>. מוצג התחקיר הקיים; לא נוצר תחקיר נוסף ולא הוקצה מספר חדש.
        </Banner>
      )}
      {numberChanged && (
        <Banner>
          המספר <Ltr>{expectedNumber}</Ltr> נתפס בינתיים בתחקיר אחר, ולכן התחקיר נשמר במספר <Ltr>{number}</Ltr>.
        </Banner>
      )}

      <div className="success-location">
        <Icon name="cloud" />
        <div>
          <span className="success-location-label">
            {created.publication.destination === 'sharePoint'
              ? 'נוצר ב-SharePoint כטופס לעריכה (סימולציה)'
              : 'נשמר בתיקייה המשותפת'}
          </span>
          <Ltr className="url">{created.publication.url}</Ltr>
        </div>
        <CopyButton value={created.publication.url} label="העתקה" />
      </div>
      <p className="success-hint">
        הטיוטה הוסרה מרשימת הטיוטות. סטטוס התחקיר: {INVESTIGATION_STATUS_LABELS[created.lifecycle.status]}.
      </p>
      {notice && <Banner tone={notice.tone}>{notice.content}</Banner>}

      <div className="success-actions">
        <Button variant="primary" size="large" icon="file" onClick={() => navigation.openInvestigation(number)}>
          פתח תחקיר
        </Button>
        <Button size="large" icon="mail" onClick={() => setDistributing(true)}>
          הפצה במייל
        </Button>
        <ExportPdfButton target={{ kind: 'investigation', number }} onNotice={setNotice} size="large" />
      </div>
      <button type="button" className="link-button success-home" onClick={navigation.goHome}>
        חזרה לדף הבית
      </button>

      {distributing && <DistributionDialog number={number} onClose={() => setDistributing(false)} />}
    </div>
  );
}
