import type { Dispatch } from 'react';

import { api } from '../../api/client';
import { errorMessage, fieldMessage } from '../../api/errors';
import type { InvestigationNumber, System } from '../../api/types';
import { rowsLabel } from '../../lib/format';
import { useResource } from '../../lib/useResource';
import { Badge, Banner, Field, Ltr, StaticField } from '../../ui/controls';
import { errorFor, type WizardAction, type WizardState } from './wizardState';

interface Props {
  state: WizardState;
  dispatch: Dispatch<WizardAction>;
  onEditRows: () => void;
}

export function DetailsStep({ state, dispatch, onEditRows }: Props) {
  const [systems] = useResource(api.listActiveSystems);
  const [nextNumber] = useResource(api.peekNextInvestigationNumber);

  const activeSystems = systems.status === 'ready' ? systems.data : [];
  const selectedSystem = activeSystems.find((system) => system.id === state.systemId);
  const message = (field: string) => {
    const error = errorFor(state, field);
    return error ? fieldMessage(error) : undefined;
  };

  return (
    <div className="form-page">
      <p className="selection-summary">
        {rowsLabel(state.selectedRowIds.size)} מתוך <Ltr>{state.log?.sourceFileName}</Ltr>
        <button type="button" className="link-button" onClick={onEditRows}>
          שינוי הבחירה
        </button>
      </p>

      {systems.status === 'error' && <Banner tone="error">{errorMessage(systems.error)}</Banner>}
      {systems.status === 'ready' && activeSystems.length === 0 && (
        <Banner tone="error">אין מערכות פעילות. מנהל מערכת צריך להגדיר או להפעיל מערכת לפני יצירת תחקיר.</Banner>
      )}

      <div className="form-card">
        <Field label="מערכת" error={message('systemId')}>
          {({ id, describedBy, invalid }) => (
            <select
              id={id}
              className="input"
              value={state.systemId}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              disabled={systems.status !== 'ready'}
              onChange={(event) => dispatch({ type: 'systemChanged', systemId: event.target.value })}
            >
              <option value="" disabled>
                {systems.status === 'loading' ? 'טוען מערכות…' : 'בחרו מערכת'}
              </option>
              {activeSystems.map((system) => (
                <option key={system.id} value={system.id}>
                  {system.name}
                </option>
              ))}
            </select>
          )}
        </Field>

        <StaticField label="מספר תחקיר">
          <NumberDisplay number={nextNumber.status === 'ready' ? nextNumber.data : null} failed={nextNumber.status === 'error'} />
        </StaticField>

        <Field
          label="קישור לבדיקות מקדימות"
          error={message('preliminaryCheckUrl')}
          hint="הדביקו את הקישור לבדיקות המקדימות שבוצעו. הקישור נשמר בתחקיר ואינו נפתח מתוך OurFault."
        >
          {({ id, describedBy, invalid }) => (
            <input
              id={id}
              className="input input-ltr"
              type="url"
              dir="ltr"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              maxLength={2048}
              placeholder="https://"
              value={state.preliminaryCheckUrl}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => dispatch({ type: 'urlChanged', url: event.target.value })}
            />
          )}
        </Field>

        <StaticField label="תבנית">
          <TemplateDisplay system={selectedSystem} />
        </StaticField>
      </div>
    </div>
  );
}

function NumberDisplay({ number, failed }: { number: InvestigationNumber | null; failed: boolean }) {
  return (
    <div className="static-value">
      <span className="investigation-number">{number ? <Ltr>{number}</Ltr> : failed ? '—' : '…'}</span>
      <Badge tone="accent">הוקצה אוטומטית</Badge>
      <p className="field-hint">המספר הבא לפי התחקירים הקיימים. הוא נשמר סופית ברגע היצירה.</p>
    </div>
  );
}

function TemplateDisplay({ system }: { system: System | undefined }) {
  if (!system) {
    return <div className="static-value static-value-muted">תיקבע לפי המערכת שתיבחר</div>;
  }
  return (
    <div className="static-value">
      <span className="static-value-main">{system.template.name}</span>
      <Badge tone="muted">לפי המערכת</Badge>
      <p className="field-hint">סעיפים: {system.template.sections.join(' · ')}</p>
    </div>
  );
}
