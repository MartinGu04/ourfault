// Step 2: the technical sections the administrator configured. Repeating
// sections are tables with rows the operator adds and removes; single-record
// sections are a form. Field order is the configured order (in RTL the first
// column is the rightmost).

import type { Dispatch } from 'react';

import { fieldMessage } from '../../api/errors';
import type { FieldDefinition, FieldValue, SectionDefinition, SectionRecord, Workspace } from '../../api/types';
import { Button, Chips, Field, Segmented, YES_NO } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { errorFor, type WizardAction, type WizardState } from './wizardState';

interface Props {
  state: WizardState;
  dispatch: Dispatch<WizardAction>;
  workspace: Workspace;
}

export function TechnicalStep({ state, dispatch, workspace }: Props) {
  if (workspace.sections.length === 0) {
    return <p className="empty-text">לא הוגדרו סעיפים טכניים. ניתן להמשיך לשלב הבא.</p>;
  }
  return (
    <div className="technical">
      {workspace.sections.map((section) => (
        <SectionCard key={section.id} section={section} state={state} dispatch={dispatch} workspace={workspace} />
      ))}
    </div>
  );
}

function SectionCard({ section, state, dispatch, workspace }: { section: SectionDefinition } & Props) {
  const records = state.sections[section.id] ?? [];
  const sectionError = errorFor(state, `sections.${section.id}`);
  const change = (index: number, fieldId: string, value: FieldValue | null) =>
    dispatch({ type: 'sectionValueChanged', sectionId: section.id, index, fieldId, value });
  const control = (field: FieldDefinition, record: SectionRecord, index: number, label: string) => (
    <FieldControl
      field={field}
      value={record[field.id]}
      label={label}
      error={errorFor(state, `sections.${section.id}.${index}.${field.id}`)}
      state={state}
      workspace={workspace}
      onChange={(value) => change(index, field.id, value)}
    />
  );

  return (
    <section className="form-card section-card" aria-labelledby={`section-${section.id}`}>
      <div className="section-card-header">
        <h2 id={`section-${section.id}`} className="form-card-title">
          {section.name}
        </h2>
        {section.mode === 'repeating' && (
          <Button icon="plus" onClick={() => dispatch({ type: 'sectionRowAdded', sectionId: section.id })}>
            הוספת שורה
          </Button>
        )}
      </div>
      {sectionError && <p className="field-error">{fieldMessage(sectionError)}</p>}

      {section.mode === 'single' ? (
        <div className="section-form">
          {section.fields.map((field) => (
            <Field
              key={field.id}
              label={field.required ? `${field.label} *` : field.label}
              error={messageFor(errorFor(state, `sections.${section.id}.0.${field.id}`))}
            >
              {() => control(field, records[0] ?? {}, 0, field.label)}
            </Field>
          ))}
        </div>
      ) : records.length === 0 ? (
        <p className="empty-text">אין שורות. הוסיפו שורה אם הסעיף רלוונטי לפעילות.</p>
      ) : (
        <div className="table-wrap">
          <table className="data-table section-table">
            <thead>
              <tr>
                <th scope="col" className="col-index">
                  #
                </th>
                {section.fields.map((field) => (
                  <th key={field.id} scope="col">
                    {field.label}
                    {field.required && <span className="required-mark"> *</span>}
                  </th>
                ))}
                <th scope="col" className="col-actions-narrow">
                  <span className="visually-hidden">פעולות</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {records.map((record, index) => {
                const rowError = errorFor(state, `sections.${section.id}.${index}`);
                return (
                  <tr key={index} className={rowError ? 'row-invalid' : undefined}>
                    <td className="col-index">{index + 1}</td>
                    {section.fields.map((field) => {
                      const error = errorFor(state, `sections.${section.id}.${index}.${field.id}`);
                      return (
                        <td key={field.id}>
                          {control(field, record, index, `${field.label}, שורה ${index + 1}`)}
                          {error && <p className="cell-error">{fieldMessage(error)}</p>}
                        </td>
                      );
                    })}
                    <td className="col-actions-narrow">
                      <button
                        type="button"
                        className="icon-button icon-button-danger"
                        aria-label={`הסרת שורה ${index + 1} מ${section.name}`}
                        title="הסרת שורה"
                        onClick={() => dispatch({ type: 'sectionRowRemoved', sectionId: section.id, index })}
                      >
                        <Icon name="trash" size={16} />
                      </button>
                      {rowError && <span className="visually-hidden">{fieldMessage(rowError)}</span>}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {records.some((_, index) => errorFor(state, `sections.${section.id}.${index}`)) && (
            <p className="field-error section-row-error">יש שורות ריקות. מלאו אותן או הסירו אותן.</p>
          )}
        </div>
      )}
    </section>
  );
}

const messageFor = (error: ReturnType<typeof errorFor>) => (error ? fieldMessage(error) : undefined);

interface ControlProps {
  field: FieldDefinition;
  value: FieldValue | undefined;
  label: string;
  error: ReturnType<typeof errorFor>;
  state: WizardState;
  workspace: Workspace;
  onChange: (value: FieldValue | null) => void;
}

/** One input for a configured field, chosen by its type. */
function FieldControl({ field, value, label, error, state, workspace, onChange }: ControlProps) {
  const invalid = Boolean(error) || undefined;
  const text = typeof value === 'string' ? value : '';
  const setText = (next: string) => onChange(next === '' ? null : next);

  switch (field.kind) {
    case 'text':
      return (
        <input className="input" aria-label={label} aria-invalid={invalid} value={text} maxLength={4000} onChange={(e) => setText(e.target.value)} />
      );
    case 'number':
      return (
        <input
          className="input input-ltr"
          dir="ltr"
          inputMode="decimal"
          aria-label={label}
          aria-invalid={invalid}
          value={text}
          maxLength={24}
          onChange={(e) => setText(e.target.value)}
        />
      );
    case 'boolean':
      return (
        <Segmented
          label={label}
          options={YES_NO}
          value={typeof value === 'boolean' ? value : null}
          invalid={Boolean(invalid)}
          onChange={(next) => onChange(next)}
        />
      );
    case 'singleSelect':
      return (
        <select className="input" aria-label={label} aria-invalid={invalid} value={text} onChange={(e) => setText(e.target.value)}>
          <option value="">בחרו…</option>
          {(field.options ?? []).map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      );
    case 'multiSelect':
      return (
        <Chips
          label={label}
          options={(field.options ?? []).map((option) => ({ value: option, label: option }))}
          selected={Array.isArray(value) ? value : []}
          invalid={Boolean(invalid)}
          onChange={(next) => onChange(next.length === 0 ? null : next)}
        />
      );
    case 'station': {
      const stations = workspace.stations.filter((station) => station.active || station.id === text);
      return (
        <select className="input" aria-label={label} aria-invalid={invalid} value={text} onChange={(e) => setText(e.target.value)}>
          <option value="">בחרו תחנה…</option>
          {stations.map((station) => (
            <option key={station.id} value={station.id} disabled={!station.active}>
              {station.active ? station.name : `${station.name} (לא פעילה)`}
            </option>
          ))}
        </select>
      );
    }
    case 'system': {
      // Only the systems involved in this activity.
      const involved = state.activity.systemIds
        .map((id) => workspace.systems.find((system) => system.id === id))
        .filter((system) => system !== undefined);
      return (
        <select className="input" aria-label={label} aria-invalid={invalid} value={text} onChange={(e) => setText(e.target.value)}>
          <option value="">{involved.length === 0 ? 'בחרו קודם מערכות בשלב 1' : 'בחרו מערכת…'}</option>
          {involved.map((system) => (
            <option key={system.id} value={system.id}>
              {system.name}
            </option>
          ))}
          {text && !involved.some((system) => system.id === text) && (
            <option value={text} disabled>
              מערכת שאינה בפעילות
            </option>
          )}
        </select>
      );
    }
  }
}
