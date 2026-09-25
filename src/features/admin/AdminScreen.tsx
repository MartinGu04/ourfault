// Admin mode: initial setup and later administration are the same screens
// over the same configuration. Every change is validated and saved by the
// backend, which only accepts it in admin mode.

import { useState } from 'react';

import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import { useResource } from '../../lib/useResource';
import { Banner, Spinner, Tabs } from '../../ui/controls';
import { PublishingPanel } from './PublishingPanel';
import { SectionsPanel } from './SectionsPanel';
import { SetupOverview } from './SetupOverview';
import { StationsPanel } from './StationsPanel';
import { SystemsPanel } from './SystemsPanel';

export type AdminTab = 'setup' | 'systems' | 'stations' | 'sections' | 'publishing';

const TABS: { key: AdminTab; label: string }[] = [
  { key: 'setup', label: 'הגדרה ראשונית' },
  { key: 'systems', label: 'מערכות ורשימות תפוצה' },
  { key: 'stations', label: 'תחנות לווייניות' },
  { key: 'sections', label: 'סעיפים טכניים' },
  { key: 'publishing', label: 'הפצה ופרסום' },
];

export function AdminScreen() {
  const [loaded, reload] = useResource(api.adminGetConfiguration);
  const [tab, setTab] = useState<AdminTab>('setup');

  return (
    <div className="page admin">
      <div className="page-heading">
        <div>
          <h1 className="page-title">הגדרות</h1>
          <p className="page-subtitle">
            ההגדרה הראשונית וההגדרות השוטפות הן אותן הגדרות: אפשר לחזור לכאן בכל עת ולשנות אותן. תחקירים שכבר נוצרו
            שומרים את הנתונים כפי שהיו בעת יצירתם.
          </p>
        </div>
      </div>
      <Tabs label="אזורי הגדרה" tabs={TABS} current={tab} onChange={setTab} />

      {loaded.status === 'loading' && <Spinner label="טוען הגדרות" />}
      {loaded.status === 'error' && <Banner tone="error">{errorMessage(loaded.error)}</Banner>}
      {loaded.status === 'ready' && (
        <div className="admin-panel" role="tabpanel">
          {tab === 'setup' && <SetupOverview setup={loaded.data.setup} onOpen={setTab} />}
          {tab === 'systems' && <SystemsPanel systems={loaded.data.configuration.systems} onChanged={reload} />}
          {tab === 'stations' && <StationsPanel stations={loaded.data.configuration.stations} onChanged={reload} />}
          {tab === 'sections' && <SectionsPanel sections={loaded.data.configuration.sections} onChanged={reload} />}
          {tab === 'publishing' && <PublishingPanel configuration={loaded.data.configuration} onChanged={reload} />}
        </div>
      )}
    </div>
  );
}
