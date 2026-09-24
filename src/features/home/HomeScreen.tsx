import { useState, type FormEvent } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import type { DraftSummary, InvestigationSummary } from '../../api/types';
import { formatDate, formatTimestamp, rowsLabel } from '../../lib/format';
import { activityTypeLabel, stepLabel } from '../../lib/labels';
import { useResource } from '../../lib/useResource';
import { Banner, Button, Ltr, Spinner } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import { Icon } from '../../ui/Icon';
import { StatusBadge } from '../investigation/StatusBadge';

export function HomeScreen({ navigation }: { navigation: Navigation }) {
  const [workspace] = useResource(api.getWorkspace);
  return (
    <div className="page page-narrow home">
      <section className="home-hero">
        <div>
          <h1 className="page-title">תחקירים</h1>
          <p className="page-subtitle">פרטי הפעילות, הפרטים הטכניים והשתלשלות האירועים – והתחקיר מוכן להפצה.</p>
        </div>
        <Button variant="primary" size="large" icon="plus" onClick={navigation.startInvestigation}>
          תחקיר חדש
        </Button>
      </section>

      {workspace.status === 'ready' && !workspace.data.setup.ready && (
        <Banner tone="error">ההגדרה הראשונית טרם הושלמה (למשל אין מערכת פעילה). מנהל צריך להשלים אותה במצב מנהל.</Banner>
      )}

      <Drafts onOpen={navigation.openDraft} />
      <SearchInvestigations onOpen={navigation.openInvestigation} />
      <RecentInvestigations onOpen={navigation.openInvestigation} />
    </div>
  );
}

function Drafts({ onOpen }: { onOpen: (id: string) => void }) {
  const [drafts, reload] = useResource(api.listDrafts);
  const [deleting, setDeleting] = useState<DraftSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function confirmDelete() {
    if (!deleting) return;
    setBusy(true);
    setError(null);
    try {
      await api.deleteDraft(deleting.id, deleting.revision);
      setDeleting(null);
      reload();
    } catch (caught) {
      setError(errorMessage(caught));
      setDeleting(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="home-section" aria-labelledby="drafts-title">
      <h2 id="drafts-title" className="section-title">
        טיוטות
      </h2>
      {error && <Banner tone="error">{error}</Banner>}
      {drafts.status === 'loading' && <Spinner label="טוען טיוטות" />}
      {drafts.status === 'error' && <Banner tone="error">{errorMessage(drafts.error)}</Banner>}
      {drafts.status === 'ready' && drafts.data.length === 0 && (
        <p className="empty-text">אין טיוטות פתוחות. תחקיר חדש נשמר כטיוטה באופן אוטומטי.</p>
      )}
      {drafts.status === 'ready' && drafts.data.length > 0 && (
        <ul className="list">
          {drafts.data.map((draft) => (
            <li key={draft.id} className="list-row">
              <button type="button" className="list-item draft-item" onClick={() => onOpen(draft.id)}>
                <Icon name="draft" className="list-icon" />
                <span className="list-main">
                  <span className="list-title">{draft.activityName || 'טיוטה ללא שם'}</span>
                  <span className="list-meta">
                    {draft.systemNames.length > 0 ? draft.systemNames.join(', ') : 'לא נבחרו מערכות'} ·{' '}
                    {rowsLabel(draft.rowCount)} · נעצר ב{stepLabel(draft.step)}
                  </span>
                </span>
                <span className="list-meta">נשמר {formatTimestamp(draft.updatedAt)}</span>
                <Icon name="forward" className="list-chevron" />
              </button>
              <button
                type="button"
                className="icon-button icon-button-danger"
                aria-label={`מחיקת הטיוטה ${draft.activityName || 'ללא שם'}`}
                title="מחיקת טיוטה"
                onClick={() => setDeleting(draft)}
              >
                <Icon name="trash" size={16} />
              </button>
            </li>
          ))}
        </ul>
      )}
      {deleting && (
        <ConfirmDialog
          title="מחיקת טיוטה"
          confirmLabel="מחיקה"
          danger
          busy={busy}
          onConfirm={confirmDelete}
          onCancel={() => setDeleting(null)}
        >
          הטיוטה <strong>{deleting.activityName || 'ללא שם'}</strong> תימחק לצמיתות. לא ניתן לשחזר אותה.
        </ConfirmDialog>
      )}
    </section>
  );
}

function SearchInvestigations({ onOpen }: { onOpen: (number: string) => void }) {
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<InvestigationSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!query.trim()) return;
    setBusy(true);
    setError(null);
    try {
      setResults(await api.searchInvestigations(query));
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="home-section" aria-labelledby="search-title">
      <h2 id="search-title" className="visually-hidden">
        חיפוש תחקיר
      </h2>
      <form className="search" role="search" onSubmit={submit}>
        <label className="visually-hidden" htmlFor="investigation-search">
          חיפוש לפי מספר תחקיר, שם משימה או מערכת
        </label>
        <div className="search-box">
          <Icon name="search" />
          <input
            id="investigation-search"
            type="search"
            autoComplete="off"
            placeholder="חיפוש לפי מספר תחקיר, שם משימה או מערכת"
            value={query}
            maxLength={120}
            onChange={(event) => {
              setQuery(event.target.value);
              setResults(null);
              setError(null);
            }}
          />
          <Button type="submit" variant="secondary" busy={busy} disabled={!query.trim()}>
            חיפוש
          </Button>
        </div>
      </form>
      {error && <p className="search-message">{error}</p>}
      {results && results.length === 0 && <p className="search-message">לא נמצאו תחקירים עבור "{query.trim()}".</p>}
      {results && results.length > 0 && <InvestigationList items={results} onOpen={onOpen} />}
    </section>
  );
}

function RecentInvestigations({ onOpen }: { onOpen: (number: string) => void }) {
  const [recent] = useResource(api.listRecentInvestigations);

  return (
    <section className="home-section" aria-labelledby="recent-title">
      <h2 id="recent-title" className="section-title">
        תחקירים אחרונים
      </h2>
      {recent.status === 'loading' && <Spinner label="טוען תחקירים" />}
      {recent.status === 'error' && <Banner tone="error">{errorMessage(recent.error)}</Banner>}
      {recent.status === 'ready' && recent.data.length === 0 && <p className="empty-text">עדיין לא נוצרו תחקירים.</p>}
      {recent.status === 'ready' && recent.data.length > 0 && <InvestigationList items={recent.data} onOpen={onOpen} />}
    </section>
  );
}

/** The dashboard columns: number | systems | activity | date (+ type, status). */
function InvestigationList({ items, onOpen }: { items: InvestigationSummary[]; onOpen: (number: string) => void }) {
  return (
    <ul className="list">
      {items.map((item) => (
        <li key={item.number} className="list-row">
          <button type="button" className="list-item investigation-item" onClick={() => onOpen(item.number)}>
            <span className="list-number">
              <Ltr>{item.number}</Ltr>
            </span>
            <span className="list-main">
              <span className="list-title">{item.activityName}</span>
              <span className="list-meta">
                {activityTypeLabel(item.activityType)} · {item.systems.join(', ')}
              </span>
            </span>
            <StatusBadge status={item.status} />
            <span className="list-meta">{formatDate(item.date)}</span>
            <Icon name="forward" className="list-chevron" />
          </button>
        </li>
      ))}
    </ul>
  );
}
