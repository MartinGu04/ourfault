// Choosing how to work. This is a work mode, not a login: there are no
// accounts or passwords, and entering admin mode does not identify anyone.
// Which modes are available is decided by the backend's access policy.
//
// The welcome screen is the brand scene: always the dark theme, whatever the
// operator chose for the rest of the application.

import { useState } from 'react';

import { errorMessage } from '../../api/errors';
import type { Session, WorkMode } from '../../api/types';
import emblem from '../../assets/brand/ourfault-emblem.webp';
import logo from '../../assets/brand/ourfault-logo.webp';
import { Backdrop } from '../../ui/Backdrop';
import { Banner, Spinner } from '../../ui/controls';
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
    <div className="entry" data-theme="dark">
      <Backdrop />
      <main className="entry-layout">
        <section className="entry-content" aria-labelledby="entry-title">
          <img className="entry-logo" src={logo} alt="OurFault" />
          <p className="entry-tagline">מערכת ליצירה וניהול תחקירים</p>
          <span className="entry-rule" aria-hidden="true" />
          <h1 id="entry-title" className="entry-title">
            ברוכים הבאים
          </h1>
          <p className="entry-subtitle">בחרו מצב עבודה כדי להמשיך למערכת</p>

          {error && <Banner tone="error">{error}</Banner>}

          <div className="entry-modes">
            <ModeCard
              icon="user"
              title="כניסה רגילה"
              description="יצירת תחקירים, צפייה בתחקירים והפצה"
              action="כניסה"
              primary
              busy={busy === 'regular'}
              disabled={busy !== null}
              onClick={() => void enter('regular')}
            />
            <ModeCard
              icon="settings"
              title="מצב מנהל"
              description="הגדרה ראשונית, מערכות, תבניות ורשימות תפוצה"
              action="כניסת מנהל"
              busy={busy === 'admin'}
              disabled={busy !== null || !session.adminAvailable}
              note={session.adminAvailable ? undefined : 'מצב מנהל אינו זמין בעמדה זו.'}
              onClick={() => void enter('admin')}
            />
          </div>

          <p className="entry-footnote">
            בחירת מצב העבודה אינה הזדהות. מצב המנהל מציג את מסכי ההגדרות; ההרשאות בסביבה האמיתית ייקבעו לפי הרשאות
            המשתמש.
          </p>
        </section>

        <div className="entry-art" aria-hidden="true">
          <span className="entry-art-glow" />
          <img className="entry-emblem" src={emblem} alt="" />
        </div>
      </main>

      <footer className="entry-status">
        <span className="entry-status-dot" aria-hidden="true" />
        <span>גרסת PoC</span>
        <span className="entry-status-sep" aria-hidden="true" />
        <span>סביבת הדגמה מקומית</span>
      </footer>
    </div>
  );
}

interface CardProps {
  icon: IconName;
  title: string;
  description: string;
  action: string;
  primary?: boolean;
  busy: boolean;
  disabled: boolean;
  note?: string | undefined;
  onClick: () => void;
}

/** A whole-card button: hover, focus and press states on one target. */
function ModeCard({ icon, title, description, action, primary, busy, disabled, note, onClick }: CardProps) {
  return (
    <button
      type="button"
      className={primary ? 'mode-card mode-card-primary' : 'mode-card'}
      disabled={disabled}
      aria-busy={busy || undefined}
      onClick={onClick}
    >
      <span className="mode-card-icon">
        <Icon name={icon} size={22} />
      </span>
      <span className="mode-card-text">
        <span className="mode-card-title">{title}</span>
        <span className="mode-card-description">{description}</span>
        {note && <span className="mode-card-note">{note}</span>}
      </span>
      <span className="mode-card-action">
        {busy ? <Spinner /> : <Icon name="enter" size={17} />}
        {action}
      </span>
    </button>
  );
}
