import { useReducer, useState } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { ApiError, errorMessage } from '../../api/errors';
import { rowsLabel } from '../../lib/format';
import { Banner, Button } from '../../ui/controls';
import { InvestigationDocument } from '../investigation/InvestigationDocument';
import { DetailsStep } from './DetailsStep';
import { SelectRowsStep } from './SelectRowsStep';
import { Stepper } from './Stepper';
import { SuccessStep } from './SuccessStep';
import { canContinueFromRows, initialWizardState, toDraft, wizardReducer } from './wizardState';

type BusyAction = 'import' | 'demo' | 'preview' | 'create';

export function NewInvestigationWizard({ navigation }: { navigation: Navigation }) {
  const [state, dispatch] = useReducer(wizardReducer, initialWizardState);
  const [busy, setBusy] = useState<BusyAction | null>(null);
  const [error, setError] = useState<string | null>(null);

  /** Runs a backend call; validation errors go to fields, others to a banner. */
  async function run(action: BusyAction, work: () => Promise<void>) {
    setBusy(action);
    setError(null);
    try {
      await work();
    } catch (caught) {
      if (caught instanceof ApiError && caught.appError.kind === 'validation') {
        dispatch({ type: 'validationFailed', errors: caught.appError.errors });
      } else {
        setError(errorMessage(caught));
      }
    } finally {
      setBusy(null);
    }
  }

  const importWorkbook = () =>
    run('import', async () => {
      const log = await api.importWorkbook();
      if (log) dispatch({ type: 'imported', log });
    });

  const importDemo = () =>
    run('demo', async () => dispatch({ type: 'imported', log: await api.importDemoWorkbook() }));

  const requestPreview = () =>
    run('preview', async () => {
      const draft = toDraft(state);
      if (draft) dispatch({ type: 'previewed', preview: await api.previewInvestigation(draft) });
    });

  const create = () =>
    run('create', async () => {
      const draft = toDraft(state);
      if (draft) dispatch({ type: 'created', investigation: await api.createInvestigation(draft) });
    });

  const goTo = (step: 'rows' | 'details') => {
    setError(null);
    dispatch({ type: 'stepRequested', step });
  };

  return (
    <div className="wizard">
      <div className="wizard-header">
        <h1 className="page-title">תחקיר חדש</h1>
        <Stepper current={state.step} />
      </div>

      <div className="wizard-body">
        {error && <Banner tone="error">{error}</Banner>}

        {state.step === 'rows' && (
          <SelectRowsStep
            state={state}
            dispatch={dispatch}
            onImport={importWorkbook}
            onImportDemo={importDemo}
            busy={busy === 'import' ? 'import' : busy === 'demo' ? 'demo' : null}
          />
        )}
        {state.step === 'details' && <DetailsStep state={state} dispatch={dispatch} onEditRows={() => goTo('rows')} />}
        {state.step === 'preview' && state.preview && (
          <div className="preview-frame">
            <InvestigationDocument investigation={state.preview} provisionalNumber />
          </div>
        )}
        {state.step === 'done' && state.created && (
          <SuccessStep
            created={state.created}
            previewNumber={state.preview?.number ?? null}
            navigation={navigation}
          />
        )}
      </div>

      {state.step !== 'done' && (
        <footer className="action-bar">
          <div className="action-bar-inner">
            {state.step === 'rows' && (
              <>
                <p className="action-bar-status" aria-live="polite">
                  {state.log
                    ? `נבחרו ${rowsLabel(state.selectedRowIds.size)} מתוך ${state.log.rows.length.toLocaleString('he-IL')}`
                    : 'יש לייבא את יומן המבצעים כדי להתחיל'}
                </p>
                <Button variant="subtle" onClick={navigation.goHome}>
                  ביטול
                </Button>
                <Button
                  variant="primary"
                  trailingIcon="forward"
                  disabled={!canContinueFromRows(state)}
                  onClick={() => goTo('details')}
                >
                  המשך
                </Button>
              </>
            )}
            {state.step === 'details' && (
              <>
                <p className="action-bar-status">{rowsLabel(state.selectedRowIds.size)} נבחרו</p>
                <Button icon="back" onClick={() => goTo('rows')}>
                  חזרה
                </Button>
                <Button variant="primary" busy={busy === 'preview'} onClick={requestPreview}>
                  תצוגה מקדימה
                </Button>
              </>
            )}
            {state.step === 'preview' && (
              <>
                <p className="action-bar-status">בדקו את התחקיר לפני היצירה.</p>
                <Button icon="back" disabled={busy === 'create'} onClick={() => goTo('details')}>
                  חזרה לעריכה
                </Button>
                <Button variant="primary" icon="check" busy={busy === 'create'} onClick={create}>
                  צור תחקיר
                </Button>
              </>
            )}
          </div>
        </footer>
      )}
    </div>
  );
}
