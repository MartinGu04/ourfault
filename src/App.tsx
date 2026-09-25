import { useEffect, useState } from 'react';

import { api } from './api/client';
import type { InvestigationNumber, Session, WorkMode } from './api/types';
import { AdminScreen } from './features/admin/AdminScreen';
import { EntryScreen } from './features/entry/EntryScreen';
import { HomeScreen } from './features/home/HomeScreen';
import { InvestigationScreen } from './features/investigation/InvestigationScreen';
import { InvestigationWizard } from './features/wizard/InvestigationWizard';
import { useResource } from './lib/useResource';
import { Button, Spinner } from './ui/controls';
import { Icon } from './ui/Icon';

export type Screen =
  | { name: 'home' }
  | { name: 'wizard'; draftId: string | null }
  | { name: 'investigation'; number: InvestigationNumber };

export interface Navigation {
  goHome: () => void;
  startInvestigation: () => void;
  openDraft: (id: string) => void;
  openInvestigation: (number: InvestigationNumber) => void;
}

export function App() {
  const [loaded] = useResource(api.getSession);
  const [session, setSession] = useState<Session | null>(null);
  const [screen, setScreen] = useState<Screen>({ name: 'home' });

  useEffect(() => {
    if (loaded.status === 'ready') setSession(loaded.data);
  }, [loaded]);

  const navigation: Navigation = {
    goHome: () => setScreen({ name: 'home' }),
    startInvestigation: () => setScreen({ name: 'wizard', draftId: null }),
    openDraft: (id) => setScreen({ name: 'wizard', draftId: id }),
    openInvestigation: (number) => setScreen({ name: 'investigation', number }),
  };

  if (loaded.status === 'error') return <StartupError />;
  if (!session) {
    return (
      <div className="app-center">
        <Spinner label="טוען" />
      </div>
    );
  }

  async function enter(mode: WorkMode) {
    setSession(await api.enterWorkMode(mode));
    setScreen({ name: 'home' });
  }

  async function leave() {
    setSession(await api.leaveWorkMode());
  }

  if (session.mode === null) {
    return <EntryScreen session={session} onEnter={enter} />;
  }

  return (
    <div className="app">
      <TopBar session={session} screen={screen} navigation={navigation} onLeave={leave} />
      <main className="app-main">
        {session.mode === 'admin' ? (
          <AdminScreen />
        ) : (
          <>
            {screen.name === 'home' && <HomeScreen navigation={navigation} />}
            {screen.name === 'wizard' && (
              <InvestigationWizard key={screen.draftId ?? 'new'} draftId={screen.draftId} navigation={navigation} />
            )}
            {screen.name === 'investigation' && (
              <InvestigationScreen number={screen.number} navigation={navigation} />
            )}
          </>
        )}
      </main>
    </div>
  );
}

interface TopBarProps {
  session: Session;
  screen: Screen;
  navigation: Navigation;
  onLeave: () => void;
}

function TopBar({ session, screen, navigation, onLeave }: TopBarProps) {
  const regular = session.mode === 'regular';
  // The wizard has its own exit (which saves the draft first).
  const inWizard = regular && screen.name === 'wizard';
  const brandIsLink = regular && screen.name === 'investigation';
  const brand = (
    <>
      <img className="brand-logo" src="/app-icon.svg" alt="" width={28} height={28} />
      <span className="brand-name">OurFault</span>
      <span className="brand-tagline">תחקירי פעילות</span>
    </>
  );
  return (
    <header className="topbar">
      {brandIsLink ? (
        <button type="button" className="brand brand-link" onClick={navigation.goHome} aria-label="דף הבית">
          {brand}
        </button>
      ) : (
        <div className="brand">{brand}</div>
      )}
      <div className="topbar-end">
        <span className={regular ? 'mode-chip' : 'mode-chip mode-chip-admin'}>
          <Icon name={regular ? 'user' : 'shield'} size={15} />
          {regular ? 'כניסה רגילה' : 'מצב מנהל'}
        </span>
        {!inWizard && (
          <Button variant="subtle" icon="swap" onClick={onLeave}>
            החלפת מצב עבודה
          </Button>
        )}
        <span className="user-chip" title="משתמש Windows">
          <span className="user-avatar" aria-hidden="true">
            {session.operatorName.trim().charAt(0).toUpperCase()}
          </span>
          <bdi>{session.operatorName}</bdi>
        </span>
      </div>
    </header>
  );
}

function StartupError() {
  return (
    <div className="app-center">
      <div className="startup-error" role="alert">
        <Icon name="alert" size={28} />
        <h1>לא ניתן לטעון את נתוני היישום</h1>
        <p>ייתכן שקובצי ההגדרות המקומיים פגומים או שאין הרשאת גישה אליהם. סגרו את היישום ופנו לתמיכה.</p>
      </div>
    </div>
  );
}
