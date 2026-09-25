import { useState, type FormEvent } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import type { DraftSummary, InvestigationSummary } from '../../api/types';
import { formatDate, formatTimestamp, rowsLabel } from '../../lib/format';
import { activityTypeLabel, stepLabel } from '../../lib/labels';
import { useResource } from '../../lib/useResource';
import { Banner, Button, EmptyState, Ltr, Spinner } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import { Icon } from '../../ui/Icon';
import { StatusBadge } from '../investigation/StatusBadge';

export function HomeScreen({ navigation }: { navigation: Navigation }) {
  const [workspace] = useResource(api.getWorkspace);
  return (
    <div className="page page-narrow home">
      <section className="home-hero" aria-labelledby="home-title">
        <div className="home-hero-text">
          <p className="page-eyebrow">OurFault · תחקירי פעילות</p>
          <h1 id="home-title" className="page-title">
            תחקירים
          </h1>
          <p className="page-subtitle">פרטי הפעילות, הפרטים הטכניים והשתלשלות האירועים – והתחקיר מוכן להפצה.</p>
        </div>
        <Button variant="primary" size="large" icon="plus" className="home-new" onClick={navigation.startInvestigation}>
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
        {drafts.status === 'ready' && drafts.data.length > 0 && <span className="count">{drafts.data.length}</span>}
      </h2>
      {error && <Banner tone="error">{error}</Banner>}
      {drafts.status === 'loading' && <Spinner label="טוען טיוטות" />}
      {drafts.status === 'error' && <Banner tone="error">{errorMessage(drafts.error)}</Banner>}
      {drafts.status === 'ready' && drafts.data.length === 0 && (
        <EmptyState icon="draft" title="אין טיוטות פתוחות" compact>
          תחקיר חדש נשמר כטיוטה באופן אוטומטי, וניתן להמשיך אותו מכאן.
        </EmptyState>
      )}
      {drafts.status === 'ready' && drafts.data.length > 0 && (
        <ul className="draft-grid">
          {drafts.data.map((draft) => (
            <li key={draft.id} className="draft-card">
              <button type="button" className="draft-item draft-card-main" onClick={() => onOpen(draft.id)}>
                <span className="draft-card-head">
                  <span className="draft-card-badge">
                    <Icon name="draft" size={14} />
                    טיוטה
                  </span>
                  <span className="draft-card-saved">
                    <Icon name="clock" size={13} />
                    נשמר {formatTimestamp(draft.updatedAt)}
                  </span>
                </span>
                <span className="draft-card-title">{draft.activityName || 'טיוטה ללא שם'}</span>
                <span className="draft-card-meta">
                  {draft.systemNames.length > 0 ? draft.systemNames.join(', ') : 'לא נבחרו מערכות'}
                </span>
                <span className="draft-card-meta">
                  {rowsLabel(draft.rowCount)} · נעצר ב{stepLabel(draft.step)}
                </span>
                <span className="draft-card-continue">
                  המשך עבודה
                  <Icon name="forward" size={16} />
                </span>
              </button>
              <button
                type="button"
                className="icon-button icon-button-danger draft-card-delete"
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
      <h2 id="search-title" className="section-title">
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
      {error && <Banner tone="error">{error}</Banner>}
      {results && results.length === 0 && (
        <EmptyState icon="search" title={`לא נמצאו תחקירים עבור "${query.trim()}"`} compact>
          ניתן לחפש לפי מספר תחקיר (למשל 056-2026), שם משימה או שם מערכת.
        </EmptyState>
      )}
      {results && results.length > 0 && (
        <div className="search-results">
          <p className="search-results-title">
            {results.length === 1 ? 'נמצא תחקיר אחד' : `נמצאו ${results.length} תחקירים`}
            <button
              type="button"
              className="link-button"
              onClick={() => {
                setQuery('');
                setResults(null);
              }}
            >
              ניקוי החיפוש
            </button>
          </p>
          <InvestigationList items={results} onOpen={onOpen} />
        </div>
      )}
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
      {recent.status === 'ready' && recent.data.length === 0 && (
        <EmptyState icon="inbox" title="עדיין לא נוצרו תחקירים" compact>
          תחקירים שייווצרו יופיעו כאן, מהחדש לישן.
        </EmptyState>
      )}
      {recent.status === 'ready' && recent.data.length > 0 && <InvestigationList items={recent.data} onOpen={onOpen} />}
    </section>
  );
}

/** The dashboard columns: number | activity (+ type) | systems | date | status. */
function InvestigationList({ items, onOpen }: { items: InvestigationSummary[]; onOpen: (number: string) => void }) {
  return (
    <div className="list">
      <div className="list-head investigation-grid" aria-hidden="true">
        <span>מספר</span>
        <span>פעילות</span>
        <span>מערכות</span>
        <span>תאריך</span>
        <span>סטטוס</span>
        <span />
      </div>
      <ul className="list-body">
        {items.map((item) => (
          <li key={item.number} className="list-row">
            <button
              type="button"
              className="list-item investigation-item investigation-grid"
              onClick={() => onOpen(item.number)}
            >
              <span className="list-number">
                <Ltr>{item.number}</Ltr>
              </span>
              <span className="list-main">
                <span className="list-title">{item.activityName}</span>
                <span className="list-meta">{activityTypeLabel(item.activityType)}</span>
              </span>
              <span className="list-systems">{item.systems.join(', ')}</span>
              <span className="list-date">{formatDate(item.date)}</span>
              <span>
                <StatusBadge status={item.status} />
              </span>
              <Icon name="forward" className="list-chevron" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
