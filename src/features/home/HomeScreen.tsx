import { useState, type FormEvent } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { ApiError, errorMessage } from '../../api/errors';
import { formatDate, rowsLabel } from '../../lib/format';
import { useResource } from '../../lib/useResource';
import { Banner, Button, Ltr, Spinner } from '../../ui/controls';
import { Icon } from '../../ui/Icon';

export function HomeScreen({ navigation }: { navigation: Navigation }) {
  return (
    <div className="page page-narrow home">
      <section className="home-hero">
        <div>
          <h1 className="page-title">תחקירים</h1>
          <p className="page-subtitle">בחרו שורות מיומן המבצעים, שייכו למערכת, והתחקיר מוכן.</p>
        </div>
        <Button variant="primary" size="large" icon="plus" onClick={navigation.startInvestigation}>
          תחקיר חדש
        </Button>
      </section>

      <SearchInvestigation onFound={navigation.openInvestigation} />
      <RecentInvestigations onOpen={navigation.openInvestigation} />
    </div>
  );
}

function SearchInvestigation({ onFound }: { onFound: (number: string) => void }) {
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!query.trim()) return;
    setBusy(true);
    setMessage(null);
    try {
      const found = await api.findInvestigation(query);
      onFound(found.number);
    } catch (error) {
      const notFound = error instanceof ApiError && error.appError.kind === 'notFound';
      setMessage(notFound ? `לא נמצא תחקיר שמספרו ${query.trim()}` : errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="search" role="search" onSubmit={submit}>
      <label className="visually-hidden" htmlFor="investigation-search">
        חיפוש תחקיר לפי מספר
      </label>
      <div className="search-box">
        <Icon name="search" />
        <input
          id="investigation-search"
          type="search"
          inputMode="numeric"
          autoComplete="off"
          placeholder="חיפוש תחקיר לפי מספר, לדוגמה 056-2026"
          value={query}
          maxLength={20}
          onChange={(event) => {
            setQuery(event.target.value);
            setMessage(null);
          }}
        />
        <Button type="submit" variant="secondary" busy={busy} disabled={!query.trim()}>
          חיפוש
        </Button>
      </div>
      {message && <p className="search-message">{message}</p>}
    </form>
  );
}

function RecentInvestigations({ onOpen }: { onOpen: (number: string) => void }) {
  const [recent] = useResource(api.listRecentInvestigations);

  return (
    <section className="recent" aria-labelledby="recent-title">
      <h2 id="recent-title" className="section-title">
        תחקירים אחרונים
      </h2>
      {recent.status === 'loading' && <Spinner label="טוען תחקירים" />}
      {recent.status === 'error' && <Banner tone="error">{errorMessage(recent.error)}</Banner>}
      {recent.status === 'ready' && recent.data.length === 0 && (
        <p className="empty-text">עדיין לא נוצרו תחקירים.</p>
      )}
      {recent.status === 'ready' && recent.data.length > 0 && (
        <ul className="recent-list">
          {recent.data.map((item) => (
            <li key={item.number}>
              <button type="button" className="recent-item" onClick={() => onOpen(item.number)}>
                <span className="recent-number">
                  <Ltr>{item.number}</Ltr>
                </span>
                <span className="recent-system">{item.systemName}</span>
                <span className="recent-meta">{rowsLabel(item.rowCount)}</span>
                <span className="recent-meta">{formatDate(item.date)}</span>
                <Icon name="forward" className="recent-chevron" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
