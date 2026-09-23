import { useState, type FormEvent } from 'react';

import { api } from '../../api/client';
import { errorMessage, fieldErrors } from '../../api/errors';
import type { System } from '../../api/types';
import { Badge, Banner, Button, Field } from '../../ui/controls';
import { emptySystemForm, FORM_FIELD_FOR, formFromSystem, inputFromForm, type SystemFormValues } from './systemForm';

interface Props {
  /** null when creating a new system. */
  system: System | null;
  notice: string | null;
  onSaved: (system: System) => void;
  onCancel?: () => void;
  onActiveChanged?: (system: System) => void;
}

type Errors = Partial<Record<keyof SystemFormValues, string>>;

export function SystemEditor({ system, notice, onSaved, onCancel, onActiveChanged }: Props) {
  const initial = system ? formFromSystem(system) : emptySystemForm;
  const [values, setValues] = useState<SystemFormValues>(initial);
  const [errors, setErrors] = useState<Errors>({});
  const [busy, setBusy] = useState<'save' | 'toggle' | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const dirty = (Object.keys(values) as (keyof SystemFormValues)[]).some((key) => values[key] !== initial[key]);

  function update(key: keyof SystemFormValues, value: string) {
    setValues((previous) => ({ ...previous, [key]: value }));
    setErrors((previous) => ({ ...previous, [key]: undefined }));
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy('save');
    setFailure(null);
    try {
      const saved = await api.adminSaveSystem(inputFromForm(system?.id ?? null, system?.active ?? true, values));
      setErrors({});
      onSaved(saved);
    } catch (caught) {
      const byField = fieldErrors(caught);
      const mapped: Errors = {};
      for (const [field, message] of Object.entries(byField)) {
        const formField = FORM_FIELD_FOR[field];
        if (formField) mapped[formField] = message;
      }
      setErrors(mapped);
      setFailure(Object.keys(mapped).length > 0 ? 'יש לתקן את השדות המסומנים.' : errorMessage(caught));
    } finally {
      setBusy(null);
    }
  }

  async function toggleActive() {
    if (!system) return;
    setBusy('toggle');
    setFailure(null);
    try {
      onActiveChanged?.(await api.adminSetSystemActive(system.id, !system.active));
    } catch (caught) {
      setFailure(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  }

  const text = (key: keyof SystemFormValues, label: string, options: { ltr?: boolean; hint?: string; max?: number } = {}) => (
    <Field label={label} error={errors[key]} hint={options.hint}>
      {({ id, describedBy, invalid }) => (
        <input
          id={id}
          className={options.ltr ? 'input input-ltr' : 'input'}
          dir={options.ltr ? 'ltr' : undefined}
          value={values[key]}
          maxLength={options.max ?? 120}
          spellCheck={!options.ltr}
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          onChange={(event) => update(key, event.target.value)}
        />
      )}
    </Field>
  );

  const multiline = (key: keyof SystemFormValues, label: string, hint: string, ltr = false) => (
    <Field label={label} error={errors[key]} hint={hint}>
      {({ id, describedBy, invalid }) => (
        <textarea
          id={id}
          className={ltr ? 'input textarea input-ltr' : 'input textarea'}
          dir={ltr ? 'ltr' : undefined}
          rows={5}
          value={values[key]}
          spellCheck={!ltr}
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          onChange={(event) => update(key, event.target.value)}
        />
      )}
    </Field>
  );

  return (
    <form className="editor" onSubmit={save} noValidate>
      <header className="editor-header">
        <div className="editor-title">
          <h2>{system ? system.name : 'מערכת חדשה'}</h2>
          {system && <Badge tone={system.active ? 'success' : 'muted'}>{system.active ? 'פעילה' : 'מושבתת'}</Badge>}
        </div>
        {system && (
          <Button busy={busy === 'toggle'} disabled={busy !== null} onClick={toggleActive}>
            {system.active ? 'השבתת המערכת' : 'הפעלת המערכת'}
          </Button>
        )}
      </header>

      <div className="editor-body">
        {notice && !failure && <Banner tone="success">{notice}</Banner>}
        {failure && <Banner tone="error">{failure}</Banner>}

        <section className="editor-section" aria-labelledby="section-general">
          <h3 id="section-general">פרטים כלליים</h3>
          {text('name', 'שם המערכת', { max: 80 })}
        </section>

        <section className="editor-section" aria-labelledby="section-template">
          <h3 id="section-template">תבנית תחקיר</h3>
          <div className="field-row">
            {text('templateName', 'שם התבנית', { max: 80 })}
            {text('templateTitle', 'כותרת המסמך')}
          </div>
          {multiline('sections', 'סעיפי התבנית', 'סעיף אחד בכל שורה. הסעיפים יופיעו בתחקיר אחרי טבלת יומן המבצעים.')}
        </section>

        <section className="editor-section" aria-labelledby="section-distribution">
          <h3 id="section-distribution">רשימת תפוצה</h3>
          {multiline('distributionList', 'נמענים', 'כתובת דוא״ל אחת בכל שורה.', true)}
        </section>

        <section className="editor-section" aria-labelledby="section-sharepoint">
          <h3 id="section-sharepoint">רשימת התחקירים ב-SharePoint</h3>
          <div className="field-row">
            {text('siteUrl', 'כתובת האתר', { ltr: true, max: 2048 })}
            {text('list', 'רשימה', { max: 80 })}
          </div>
        </section>
      </div>

      <footer className="editor-footer">
        {onCancel && (
          <Button variant="subtle" onClick={onCancel} disabled={busy !== null}>
            ביטול
          </Button>
        )}
        {!onCancel && dirty && (
          <Button variant="subtle" onClick={() => setValues(initial)} disabled={busy !== null}>
            ביטול השינויים
          </Button>
        )}
        <Button type="submit" variant="primary" busy={busy === 'save'} disabled={busy !== null || (!!system && !dirty)}>
          {system ? 'שמירה' : 'יצירת מערכת'}
        </Button>
      </footer>
    </form>
  );
}
