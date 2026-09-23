import { useState } from 'react';

import { api } from './api/client';
import type { CurrentUser, InvestigationNumber } from './api/types';
import { AdminScreen } from './features/admin/AdminScreen';
import { HomeScreen } from './features/home/HomeScreen';
import { InvestigationScreen } from './features/investigation/InvestigationScreen';
import { NewInvestigationWizard } from './features/wizard/NewInvestigationWizard';
import { useResource } from './lib/useResource';
import { Button, Spinner } from './ui/controls';
import { Icon } from './ui/Icon';

export type Screen =
  | { name: 'home' }
  | { name: 'wizard' }
  | { name: 'investigation'; number: InvestigationNumber }
  | { name: 'admin' };

export interface Navigation {
  goHome: () => void;
  startInvestigation: () => void;
  openInvestigation: (number: InvestigationNumber) => void;
  openAdmin: () => void;
}

export function App() {
  const [session] = useResource(api.getSession);
  const [screen, setScreen] = useState<Screen>({ name: 'home' });

  const navigation: Navigation = {
    goHome: () => setScreen({ name: 'home' }),
    startInvestigation: () => setScreen({ name: 'wizard' }),
    openInvestigation: (number) => setScreen({ name: 'investigation', number }),
    openAdmin: () => setScreen({ name: 'admin' }),
  };

  if (session.status === 'loading') {
    return (
      <div className="app-center">
        <Spinner label="טוען" />
      </div>
    );
  }
  if (session.status === 'error') {
    return <StartupError />;
  }

  const user = session.data;
  return (
    <div className="app">
      <TopBar user={user} screen={screen} navigation={navigation} />
      <main className="app-main">
        {screen.name === 'home' && <HomeScreen navigation={navigation} />}
        {screen.name === 'wizard' && <NewInvestigationWizard navigation={navigation} />}
        {screen.name === 'investigation' && <InvestigationScreen number={screen.number} navigation={navigation} />}
        {screen.name === 'admin' && user.isAdmin && <AdminScreen navigation={navigation} />}
      </main>
    </div>
  );
}

function TopBar({ user, screen, navigation }: { user: CurrentUser; screen: Screen; navigation: Navigation }) {
  // Leaving the wizard from the top bar would silently discard work, so the
  // brand only navigates home from other screens.
  const brandIsLink = screen.name !== 'wizard' && screen.name !== 'home';
  const brand = (
    <>
      <img className="brand-logo" src="/app-icon.svg" alt="" width={28} height={28} />
      <span className="brand-name">OurFault</span>
      <span className="brand-tagline">תחקירים מיומן המבצעים</span>
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
        {user.isAdmin && screen.name === 'home' && (
          <Button variant="subtle" icon="settings" onClick={navigation.openAdmin}>
            ניהול מערכות
          </Button>
        )}
        <span className="user-chip" title={user.isAdmin ? 'מנהל מערכת' : 'מפעיל'}>
          <span className="user-avatar" aria-hidden="true">
            {user.displayName.trim().charAt(0)}
          </span>
          {user.displayName}
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
