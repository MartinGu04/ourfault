import { useState } from 'react';

import { api } from '../../api/client';
import type { System } from '../../api/types';
import { Badge, Banner, Button, Field } from '../../ui/controls';
import { emptySystemForm, formFromSystem, inputFromForm, type SystemFormValues } from './systemForm';
import { useSave } from './useSave';

type Selection = { mode: 'edit'; id: string } | { mode: 'new' };

export function SystemsPanel({ systems, onChanged }: { systems: System[]; onChanged: () => void }) {
  const [selection, setSelection] = useState<Selection | null>(null);
  const effective = selection ?? (systems[0] ? { mode: 'edit' as const, id: systems[0].id } : { mode: 'new' as const });
  const selected = effective.mode === 'edit' ? systems.find((s) => s.id === effective.id) : undefined;

  return (
    <div className="admin-layout">
      <nav className="item-list" aria-label="מערכות">
        {systems.map((system) => (
          <button
            key={system.id}
            type="button"
            className="item-list-entry"
            aria-current={selected?.id === system.id ? 'true' : undefined}
            onClick={() => setSelection({ mode: 'edit', id: system.id })}
          >
            <span className="item-list-name">{system.name}</span>
            <Badge tone={system.active ? 'success' : 'muted'}>{system.active ? 'פעילה' : 'מושבתת'}</Badge>
            <span className="item-list-meta">
              {system.distributionList.length > 0 ? `${system.distributionList.length} נמענים` : 'אין רשימת תפוצה'}
            </span>
          </button>
        ))}
        <Button icon="plus" onClick={() => setSelection({ mode: 'new' })}>
          מערכת חדשה
        </Button>
      </nav>
      <SystemEditor
        key={selected?.id ?? 'new'}
        system={selected ?? null}
        onSaved={(id) => {
          setSelection({ mode: 'edit', id });
          onChanged();
        }}
      />
    </div>
  );
}

function SystemEditor({ system, onSaved }: { system: System | null; onSaved: (id: string) => void }) {
  const initial = system ? formFromSystem(system) : emptySystemForm;
  const [values, setValues] = useState<SystemFormValues>(initial);
  const { busy, errors, failure, notice, save } = useSave();
  const dirty = values.name !== initial.name || values.distributionList !== initial.distributionList;

  async function submit() {
    let savedId = system?.id ?? '';
    const ok = await save(async () => {
      savedId = (await api.adminSaveSystem(inputFromForm(system?.id ?? null, values))).id;
    }, system ? 'השינויים נשמרו.' : 'המערכת נוצרה.');
    if (ok) onSaved(savedId);
  }

  async function toggle() {
    if (!system) return;
    const ok = await save(
      () => api.adminSetSystemActive(system.id, !system.active),
      system.active ? 'המערכת הושבתה. היא לא תוצע בתחקירים חדשים; ההיסטוריה שלה נשמרת.' : 'המערכת הופעלה.',
    );
    if (ok) onSaved(system.id);
  }

  return (
    <form
      className="editor"
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <header className="editor-header">
        <div className="editor-title">
          <h2>{system ? system.name : 'מערכת חדשה'}</h2>
          {system && <Badge tone={system.active ? 'success' : 'muted'}>{system.active ? 'פעילה' : 'מושבתת'}</Badge>}
        </div>
        {system && (
          <Button disabled={busy} onClick={toggle}>
            {system.active ? 'השבתת המערכת' : 'הפעלת המערכת'}
          </Button>
        )}
      </header>
      <div className="editor-body">
        {notice && !failure && <Banner tone="success">{notice}</Banner>}
        {failure && <Banner tone="error">{failure}</Banner>}
        <Field label="שם המערכת" error={errors.name}>
          {({ id, describedBy, invalid }) => (
            <input
              id={id}
              className="input"
              value={values.name}
              maxLength={80}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => setValues({ ...values, name: event.target.value })}
            />
          )}
        </Field>
        <Field
          label="רשימת תפוצה"
          error={errors.distributionList}
          hint="כתובת דוא״ל אחת בכל שורה. בהפצת תחקיר נשלחת הודעה לכל הנמענים של כל המערכות בתחקיר, ללא כפילויות."
        >
          {({ id, describedBy, invalid }) => (
            <textarea
              id={id}
              className="input textarea input-ltr"
              dir="ltr"
              rows={6}
              spellCheck={false}
              value={values.distributionList}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => setValues({ ...values, distributionList: event.target.value })}
            />
          )}
        </Field>
        <p className="field-hint">מערכות אינן נמחקות: מערכת עם היסטוריית תחקירים מושבתת בלבד.</p>
      </div>
      <footer className="editor-footer">
        {dirty && system && (
          <Button variant="subtle" disabled={busy} onClick={() => setValues(initial)}>
            ביטול השינויים
          </Button>
        )}
        <Button type="submit" variant="primary" busy={busy} disabled={busy || (!!system && !dirty)}>
          {system ? 'שמירה' : 'יצירת מערכת'}
        </Button>
      </footer>
    </form>
  );
}
