// The investigation workflow. Everything the operator enters is kept in a
// draft that is saved automatically; the operator can leave at any time and
// continue later. Completing the draft allocates the final number.

import { useEffect, useMemo, useReducer, useRef, useState } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { ApiError, errorMessage, PASTE_TOO_LARGE_MESSAGE } from '../../api/errors';
import type { Draft, DraftContent, DraftStep, Review, Workspace } from '../../api/types';
import { rowsLabel } from '../../lib/format';
import { describeField, STEPS } from '../../lib/labels';
import { useResource } from '../../lib/useResource';
import { Banner, Button, Spinner } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/ConfirmDialog';
import { ExportPdfButton, type Notice } from '../investigation/ExportPdfButton';
import { ActivityStep } from './ActivityStep';
import { Autosaver, type SaveStatus } from './autosave';
import { AutosaveIndicator } from './AutosaveIndicator';
import { NOT_TABULAR_MESSAGE, type ClipboardRead } from './clipboard';
import { PasteRowsStep } from './PasteRowsStep';
import { ReviewStep } from './ReviewStep';
import { Stepper } from './Stepper';
import { SuccessStep } from './SuccessStep';
import { TechnicalStep } from './TechnicalStep';
import { stateFromDraft, toContent, visibleErrors, wizardReducer } from './wizardState';

interface Props {
  /** null starts a new investigation. */
  draftId: string | null;
  navigation: Navigation;
}

export function InvestigationWizard({ draftId, navigation }: Props) {
  const [draft] = useResource(() => (draftId ? api.getDraft(draftId) : Promise.resolve(null)));
  const [workspace] = useResource(api.getWorkspace);

  const failure = draft.status === 'error' ? draft.error : workspace.status === 'error' ? workspace.error : null;
  if (failure) {
    return (
      <div className="page page-narrow">
        <Banner tone="error">{errorMessage(failure)}</Banner>
        <Button icon="back" onClick={navigation.goHome}>
          חזרה לדף הבית
        </Button>
      </div>
    );
  }
  if (draft.status !== 'ready' || workspace.status !== 'ready') {
    return (
      <div className="app-center">
        <Spinner label="טוען" />
      </div>
    );
  }
  return <Wizard initialDraft={draft.data} workspace={workspace.data} navigation={navigation} />;
}

type BusyAction = 'paste' | 'complete' | 'delete' | 'exit';

interface Snapshot {
  content: DraftContent;
  step: DraftStep;
}

const isEmpty = (content: DraftContent) =>
  JSON.stringify(content) === JSON.stringify(toContent(stateFromDraft(null)));

function Wizard({ initialDraft, workspace, navigation }: { initialDraft: Draft | null; workspace: Workspace; navigation: Navigation }) {
  const [state, dispatch] = useReducer(wizardReducer, initialDraft, stateFromDraft);
  const [busy, setBusy] = useState<BusyAction | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [saveStatus, setSaveStatus] = useState<SaveStatus>(initialDraft ? 'saved' : 'idle');
  const [saveError, setSaveError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<'exit' | 'delete' | null>(null);
  const [review, setReview] = useState<Review | null>(null);

  // The saved draft this wizard writes to (null until the first save).
  const draftRef = useRef<{ id: string; revision: number } | null>(
    initialDraft ? { id: initialDraft.id, revision: initialDraft.revision } : null,
  );
  const saverRef = useRef<Autosaver<Snapshot> | null>(null);

  useEffect(() => {
    const saver = new Autosaver<Snapshot>({
      persist: async ({ content, step }) => {
        const current = draftRef.current;
        const saved = current
          ? await api.saveDraft(current.id, current.revision, content, step)
          : await api.createDraft(content, step);
        draftRef.current = { id: saved.id, revision: saved.revision };
      },
      onStatus: (status, failure) => {
        setSaveStatus(status);
        setSaveError(status === 'error' ? errorMessage(failure) : null);
      },
      // Another workstation changed the draft: never overwrite it.
      isFatal: (failure) => failure instanceof ApiError && failure.appError.kind === 'conflict',
    });
    saverRef.current = saver;
    return () => saver.dispose();
  }, []);

  const content = useMemo(
    () => toContent(state),
    [state.activity, state.sections, state.rows, state.preliminaryCheckUrl],
  );

  // Autosave whenever the content or the step changes (never for an
  // untouched new investigation, and never after completion).
  const lastQueued = useRef(JSON.stringify({ content, step: state.step }));
  useEffect(() => {
    if (state.step === 'done') return;
    const snapshot = { content, step: state.step };
    const json = JSON.stringify(snapshot);
    if (json === lastQueued.current) return;
    lastQueued.current = json;
    if (!draftRef.current && isEmpty(content)) return;
    saverRef.current?.update(snapshot);
  }, [content, state.step]);

  // Findings for the content being edited, from the same backend rules as
  // the review (local and cheap: no number lookup, no rendering, no save).
  // Only the latest answer counts.
  const assessRun = useRef(0);
  useEffect(() => {
    if (state.step === 'done') return;
    const current = ++assessRun.current;
    const timer = window.setTimeout(() => {
      api.assessDraft(content).then(
        (advisories) => {
          if (current === assessRun.current) dispatch({ type: 'assessed', advisories });
        },
        // Inline findings are a convenience; the review step reports failures.
        () => undefined,
      );
    }, 200);
    return () => window.clearTimeout(timer);
  }, [content]);

  async function saveNow(): Promise<boolean> {
    return (await saverRef.current?.flush()) ?? true;
  }

  /** Runs a backend call; validation errors go to fields, others to a banner. */
  async function run(action: BusyAction, work: () => Promise<void>) {
    setBusy(action);
    setError(null);
    try {
      await work();
    } catch (caught) {
      if (caught instanceof ApiError && caught.appError.kind === 'validation') {
        dispatch({ type: 'validationFailed', errors: caught.appError.errors });
        setError('לא ניתן ליצור את התחקיר: יש פרטים חסרים או שגויים.');
      } else {
        setError(errorMessage(caught));
      }
    } finally {
      setBusy(null);
    }
  }

  const pasteText = (read: ClipboardRead) => {
    if (busy) return;
    if (read.kind === 'rejected') {
      setError(read.reason === 'too_large' ? PASTE_TOO_LARGE_MESSAGE : NOT_TABULAR_MESSAGE);
      return;
    }
    void run('paste', async () => dispatch({ type: 'rowsPasted', pasted: await api.parsePastedRows(read.text) }));
  };

  const goTo = (step: DraftStep) => {
    setError(null);
    dispatch({ type: 'stepRequested', step });
    window.scrollTo({ top: 0 });
  };

  const complete = () =>
    run('complete', async () => {
      if (!(await saveNow()) || !draftRef.current) {
        setError('לא ניתן לשמור את הטיוטה, ולכן התחקיר לא נוצר. נסו שוב.');
        return;
      }
      const { id, revision } = draftRef.current;
      const completion = await api.completeDraft(id, revision);
      saverRef.current?.dispose();
      dispatch({ type: 'created', completion });
    });

  async function exit(force = false) {
    setBusy('exit');
    const saved = await saveNow();
    setBusy(null);
    if (saved || force) {
      navigation.goHome();
    } else {
      setConfirm('exit');
    }
  }

  const deleteDraft = () =>
    run('delete', async () => {
      await saveNow();
      saverRef.current?.dispose();
      if (draftRef.current) await api.deleteDraft(draftRef.current.id, draftRef.current.revision);
      navigation.goHome();
    });

  const stepIndex = STEPS.findIndex((step) => step.key === state.step);
  const shownErrors = useMemo(() => visibleErrors(state), [state.fieldErrors, state.assessment, state.touched, state.attempted]);
  const issuesPerStep = useMemo(() => {
    const counts: Partial<Record<DraftStep, number>> = {};
    for (const issue of shownErrors) {
      const { step } = describeField(issue.field, workspace.sections);
      counts[step] = (counts[step] ?? 0) + 1;
    }
    return counts;
  }, [shownErrors, workspace.sections]);

  if (state.step === 'done' && state.created) {
    return (
      <div className="wizard">
        <div className="wizard-body">
          <SuccessStep
            created={state.created.investigation}
            alreadyExisted={state.created.alreadyExisted}
            expectedNumber={state.expectedNumber}
            navigation={navigation}
          />
        </div>
      </div>
    );
  }
  const step = state.step === 'done' ? 'review' : state.step;
  const next = STEPS[stepIndex + 1]?.key;
  const previous = STEPS[stepIndex - 1]?.key;
  const title = state.activity.name.trim() || (initialDraft ? 'טיוטה ללא שם' : 'תחקיר חדש');

  return (
    <div className="wizard">
      <div className="wizard-header">
        <div className="wizard-title">
          <h1 className="page-title">{title}</h1>
          <AutosaveIndicator status={saveStatus} error={saveError} onRetry={() => void saveNow()} />
        </div>
        <Stepper current={step} issues={issuesPerStep} onSelect={goTo} />
      </div>

      <div className="wizard-body">
        {!workspace.setup.ready && (
          <Banner tone="error">ההגדרה הראשונית טרם הושלמה. ניתן לשמור טיוטה, אך לא ליצור תחקיר.</Banner>
        )}
        {error && <Banner tone="error">{error}</Banner>}
        {notice && <Banner tone={notice.tone}>{notice.content}</Banner>}

        {step === 'activity' && <ActivityStep state={state} dispatch={dispatch} workspace={workspace} />}
        {step === 'technical' && <TechnicalStep state={state} dispatch={dispatch} workspace={workspace} />}
        {step === 'chronology' && (
          <PasteRowsStep state={state} dispatch={dispatch} onPasteText={pasteText} parsing={busy === 'paste'} />
        )}
        {step === 'review' && (
          <ReviewStep
            state={state}
            content={content}
            dispatch={dispatch}
            workspace={workspace}
            onGoTo={goTo}
            onReview={setReview}
          />
        )}
      </div>

      <footer className="action-bar">
        <div className="action-bar-inner">
          <Button variant="subtle" busy={busy === 'exit'} disabled={busy !== null && busy !== 'exit'} onClick={() => void exit()}>
            שמירה ויציאה
          </Button>
          {(draftRef.current || initialDraft) && (
            <Button variant="danger" icon="trash" disabled={busy !== null} onClick={() => setConfirm('delete')}>
              מחיקת טיוטה
            </Button>
          )}
          <p className="action-bar-status" aria-live="polite">
            {step === 'chronology' &&
              (state.rows.length > 0 ? `${rowsLabel(state.rows.length)} בהשתלשלות` : 'העתיקו שורות מ-Excel והדביקו אותן')}
          </p>
          {previous && (
            <Button icon="back" disabled={busy !== null} onClick={() => goTo(previous)}>
              חזרה
            </Button>
          )}
          {step === 'review' && (
            <ExportPdfButton
              label="ייצוא טיוטה ל-PDF"
              target={{ kind: 'draft', prepare: async () => ((await saveNow()) ? (draftRef.current?.id ?? null) : null) }}
              onNotice={setNotice}
            />
          )}
          {next ? (
            <Button variant="primary" trailingIcon="forward" disabled={busy !== null} onClick={() => goTo(next)}>
              המשך
            </Button>
          ) : (
            <Button
              variant="primary"
              icon="check"
              busy={busy === 'complete'}
              disabled={busy !== null || !review || state.fieldErrors.length > 0 || !workspace.setup.ready}
              onClick={complete}
            >
              יצירת תחקיר
            </Button>
          )}
        </div>
      </footer>

      {confirm === 'exit' && (
        <ConfirmDialog
          title="השינויים האחרונים לא נשמרו"
          confirmLabel="יציאה בכל זאת"
          danger
          onConfirm={() => {
            setConfirm(null);
            void exit(true);
          }}
          onCancel={() => setConfirm(null)}
        >
          {saveError ?? 'לא ניתן היה לשמור את הטיוטה.'} אם תצאו עכשיו, השינויים האחרונים יאבדו.
        </ConfirmDialog>
      )}
      {confirm === 'delete' && (
        <ConfirmDialog
          title="מחיקת טיוטה"
          confirmLabel="מחיקה"
          danger
          busy={busy === 'delete'}
          onConfirm={() => {
            setConfirm(null);
            void deleteDraft();
          }}
          onCancel={() => setConfirm(null)}
        >
          הטיוטה תימחק לצמיתות. לא ניתן לשחזר אותה.
        </ConfirmDialog>
      )}
    </div>
  );
}
