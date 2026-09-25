// Choosing how to work. This is a work mode, not a login: there are no
// accounts or passwords, and entering admin mode does not identify anyone.
// Which modes are available is decided by the backend's access policy.

import { useState } from 'react';

import { errorMessage } from '../../api/errors';
import type { Session, WorkMode } from '../../api/types';
import { Banner, Button } from '../../ui/controls';
import { Icon, type IconName } from '../../ui/Icon';

interface Props {
  session: Session;
  onEnter: (mode: WorkMode) => Promise<void>;
}

export function EntryScreen({ session, onEnter }: Props) {
  const [busy, setBusy] = useState<WorkMode | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function enter(mode: WorkMode) {
    setBusy(mode);
    setError(null);
    try {
      await onEnter(mode);
    } catch (caught) {
      setError(errorMessage(caught));
      setBusy(null);
    }
  }

  return (
    <div className="entry">
      <div className="entry-brand">
        <img src="/app-icon.svg" alt="" width={56} height={56} />
        <h1>OurFault</h1>
        <p>תחקירי פעילות – מיומן המבצעים ועד להפצה</p>
      </div>
      {error && <Banner tone="error">{error}</Banner>}
      <div className="entry-options">
        <EntryOption
          icon="user"
          title="כניסה רגילה"
          description="יצירת תחקירים, צפייה בתחקירים והפצה"
          action="כניסה"
          primary
          busy={busy === 'regular'}
          onClick={() => enter('regular')}
        />
        <EntryOption
          icon="shield"
          title="מצב מנהל"
          description="הגדרה ראשונית, מערכות, תבניות ורשימות תפוצה"
          action="כניסת מנהל"
          busy={busy === 'admin'}
          disabled={!session.adminAvailable}
          note={session.adminAvailable ? undefined : 'מצב מנהל אינו זמין בעמדה זו.'}
          onClick={() => enter('admin')}
        />
      </div>
      <p className="entry-footnote">
        בחירת מצב העבודה אינה הזדהות. מצב המנהל מציג את מסכי ההגדרות; ההרשאות בסביבה האמיתית ייקבעו לפי הרשאות
        המשתמש.
      </p>
    </div>
  );
}

interface OptionProps {
  icon: IconName;
  title: string;
  description: string;
  action: string;
  primary?: boolean;
  busy: boolean;
  disabled?: boolean;
  note?: string | undefined;
  onClick: () => void;
}

function EntryOption({ icon, title, description, action, primary, busy, disabled, note, onClick }: OptionProps) {
  return (
    <section className="entry-option" aria-label={title}>
      <span className="entry-option-icon">
        <Icon name={icon} size={24} />
      </span>
      <h2>{title}</h2>
      <p>{description}</p>
      {note && <p className="entry-option-note">{note}</p>}
      <Button variant={primary ? 'primary' : 'secondary'} size="large" busy={busy} disabled={disabled} onClick={onClick}>
        {action}
      </Button>
    </section>
  );
}
