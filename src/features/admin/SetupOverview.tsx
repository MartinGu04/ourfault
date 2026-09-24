import type { SetupStatus } from '../../api/types';
import { SETUP_ITEMS } from '../../lib/labels';
import { Badge, Banner } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import type { AdminTab } from './AdminScreen';

const TAB_FOR: Record<string, AdminTab> = {
  systems: 'systems',
  distributionLists: 'systems',
  stations: 'stations',
  sections: 'sections',
  mailTemplate: 'publishing',
  publication: 'publishing',
};

/** The setup checklist, computed by the backend from the current configuration. */
export function SetupOverview({ setup, onOpen }: { setup: SetupStatus; onOpen: (tab: AdminTab) => void }) {
  return (
    <div className="setup">
      {setup.ready ? (
        <Banner tone="success">ההגדרה הראשונית הושלמה. ניתן ליצור תחקירים, ולעדכן את ההגדרות בכל עת.</Banner>
      ) : (
        <Banner tone="error">חסרות הגדרות חובה. עד להשלמתן ניתן לשמור טיוטות אך לא ליצור תחקירים.</Banner>
      )}
      <ul className="setup-list">
        {setup.items.map((item) => {
          const text = SETUP_ITEMS[item.key] ?? { title: item.key, hint: '' };
          return (
            <li key={item.key}>
              <button type="button" className="setup-item" onClick={() => onOpen(TAB_FOR[item.key] ?? 'setup')}>
                <span className={item.done ? 'setup-mark setup-mark-done' : 'setup-mark'}>
                  <Icon name={item.done ? 'check' : 'alert'} size={16} />
                </span>
                <span className="setup-text">
                  <span className="setup-title">{text.title}</span>
                  <span className="setup-hint">{text.hint}</span>
                </span>
                <Badge tone={item.required ? 'accent' : 'muted'}>{item.required ? 'חובה' : 'מומלץ'}</Badge>
                <Icon name="forward" className="list-chevron" />
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
