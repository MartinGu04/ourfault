// The section builder: structured technical sections, not a free-form
// editor. Sections and fields are ordered with move up / move down.

import { useState } from 'react';

import { api } from '../../api/client';
import type { FieldKind, SectionDefinition } from '../../api/types';
import { FIELD_KIND_LABELS } from '../../lib/labels';
import { Badge, Banner, Button, Field, Segmented } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import {
  addField,
  emptySectionForm,
  formFromSection,
  hasOptions,
  inputFromForm,
  moveField,
  removeField,
  updateField,
  type SectionForm,
} from './sectionForm';
import { useSave } from './useSave';

type Selection = { mode: 'edit'; id: string } | { mode: 'new' };

export function SectionsPanel({ sections, onChanged }: { sections: SectionDefinition[]; onChanged: () => void }) {
  const [selection, setSelection] = useState<Selection | null>(null);
  const { busy, failure, save } = useSave();
  const effective = selection ?? (sections[0] ? { mode: 'edit' as const, id: sections[0].id } : { mode: 'new' as const });
  const selected = effective.mode === 'edit' ? sections.find((s) => s.id === effective.id) : undefined;

  async function move(id: string, offset: -1 | 1) {
    if (await save(() => api.adminMoveSection(id, offset), 'הסדר עודכן.')) onChanged();
  }

  return (
    <div className="admin-layout">
      <nav className="item-list" aria-label="סעיפים טכניים">
        {failure && <Banner tone="error">{failure}</Banner>}
        {sections.map((section, index) => (
          <div key={section.id} className="item-list-row">
            <button
              type="button"
              className="item-list-entry"
              aria-current={selected?.id === section.id ? 'true' : undefined}
              onClick={() => setSelection({ mode: 'edit', id: section.id })}
            >
              <span className="item-list-name">
                {index + 1}. {section.name}
              </span>
              <Badge tone={section.active ? 'success' : 'muted'}>{section.active ? 'פעיל' : 'מושבת'}</Badge>
              <span className="item-list-meta">
                {section.mode === 'repeating' ? 'טבלה עם שורות' : 'רשומה אחת'} · {section.fields.length} שדות
              </span>
            </button>
            <div className="order-buttons">
              <button
                type="button"
                className="icon-button"
                aria-label={`הזזת ${section.name} למעלה`}
                disabled={busy || index === 0}
                onClick={() => void move(section.id, -1)}
              >
                <Icon name="up" size={16} />
              </button>
              <button
                type="button"
                className="icon-button"
                aria-label={`הזזת ${section.name} למטה`}
                disabled={busy || index === sections.length - 1}
                onClick={() => void move(section.id, 1)}
              >
                <Icon name="down" size={16} />
              </button>
            </div>
          </div>
        ))}
        <Button icon="plus" onClick={() => setSelection({ mode: 'new' })}>
          סעיף חדש
        </Button>
      </nav>
      <SectionEditor
        key={selected ? `${selected.id}-${JSON.stringify(selected)}` : 'new'}
        section={selected ?? null}
        onSaved={(id) => {
          setSelection({ mode: 'edit', id });
          onChanged();
        }}
      />
    </div>
  );
}

const KIND_OPTIONS = (Object.keys(FIELD_KIND_LABELS) as FieldKind[]).map((kind) => ({
  value: kind,
  label: FIELD_KIND_LABELS[kind],
}));

function SectionEditor({ section, onSaved }: { section: SectionDefinition | null; onSaved: (id: string) => void }) {
  const [form, setForm] = useState<SectionForm>(() => (section ? formFromSection(section) : emptySectionForm()));
  const { busy, errors, failure, notice, save } = useSave();

  async function submit() {
    let savedId = section?.id ?? '';
    const ok = await save(async () => {
      savedId = (await api.adminSaveSection(inputFromForm(form))).id;
    }, section ? 'הסעיף נשמר.' : 'הסעיף נוצר.');
    if (ok) onSaved(savedId);
  }

  async function toggle() {
    if (!section) return;
    const done = section.active ? 'הסעיף הושבת ולא יוצג בתחקירים חדשים.' : 'הסעיף הופעל.';
    if (await save(() => api.adminSetSectionActive(section.id, !section.active), done)) onSaved(section.id);
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
          <h2>{section ? section.name : 'סעיף חדש'}</h2>
          {section && <Badge tone={section.active ? 'success' : 'muted'}>{section.active ? 'פעיל' : 'מושבת'}</Badge>}
        </div>
        {section && (
          <Button disabled={busy} onClick={() => void toggle()}>
            {section.active ? 'השבתת הסעיף' : 'הפעלת הסעיף'}
          </Button>
        )}
      </header>
      <div className="editor-body">
        {notice && !failure && <Banner tone="success">{notice}</Banner>}
        {failure && <Banner tone="error">{failure}</Banner>}
        <div className="field-row">
          <Field label="שם הסעיף" error={errors.name}>
            {({ id, describedBy, invalid }) => (
              <input
                id={id}
                className="input"
                value={form.name}
                maxLength={80}
                aria-describedby={describedBy}
                aria-invalid={invalid || undefined}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
              />
            )}
          </Field>
          <Field label="מבנה">
            {() => (
              <Segmented
                label="מבנה הסעיף"
                options={[
                  { value: 'repeating', label: 'טבלה עם שורות חוזרות' },
                  { value: 'single', label: 'רשומה אחת' },
                ]}
                value={form.mode}
                onChange={(mode) => setForm({ ...form, mode })}
              />
            )}
          </Field>
        </div>

        <div className="field-editor">
          <div className="field-editor-head">
            <h3>שדות (לפי סדר התצוגה)</h3>
            {errors.fields && <p className="field-error">{errors.fields}</p>}
          </div>
          <ol className="field-editor-list">
            {form.fields.map((field, index) => {
              const prefix = `fields.${index}`;
              const set = (patch: Parameters<typeof updateField>[2]) => setForm(updateField(form, index, patch));
              return (
                <li key={field.key} className={field.active ? 'field-editor-row' : 'field-editor-row is-inactive'}>
                  <span className="field-editor-index">{index + 1}</span>
                  <div className="field-editor-main">
                    <div className="field-editor-line">
                      <input
                        className="input"
                        aria-label={`כותרת שדה ${index + 1}`}
                        placeholder="כותרת השדה"
                        value={field.label}
                        maxLength={80}
                        aria-invalid={Boolean(errors[`${prefix}.label`]) || undefined}
                        onChange={(event) => set({ label: event.target.value })}
                      />
                      <select
                        className="input field-kind"
                        aria-label={`סוג שדה ${index + 1}`}
                        value={field.kind}
                        onChange={(event) => set({ kind: event.target.value as FieldKind })}
                      >
                        {KIND_OPTIONS.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <label className="checkbox">
                        <input type="checkbox" checked={field.required} onChange={(e) => set({ required: e.target.checked })} />
                        חובה
                      </label>
                      <label className="checkbox">
                        <input type="checkbox" checked={field.active} onChange={(e) => set({ active: e.target.checked })} />
                        פעיל
                      </label>
                    </div>
                    {errors[`${prefix}.label`] && <p className="field-error">{errors[`${prefix}.label`]}</p>}
                    {errors[prefix] && <p className="field-error">{errors[prefix]}</p>}
                    {hasOptions(field.kind) && (
                      <>
                        <textarea
                          className="input textarea field-options"
                          aria-label={`אפשרויות לשדה ${index + 1}`}
                          placeholder="אפשרות אחת בכל שורה"
                          rows={3}
                          value={field.options}
                          onChange={(event) => set({ options: event.target.value })}
                        />
                        {errors[`${prefix}.options`] && <p className="field-error">{errors[`${prefix}.options`]}</p>}
                      </>
                    )}
                  </div>
                  <div className="order-buttons">
                    <button
                      type="button"
                      className="icon-button"
                      aria-label={`הזזת שדה ${index + 1} למעלה`}
                      disabled={index === 0}
                      onClick={() => setForm(moveField(form, index, -1))}
                    >
                      <Icon name="up" size={16} />
                    </button>
                    <button
                      type="button"
                      className="icon-button"
                      aria-label={`הזזת שדה ${index + 1} למטה`}
                      disabled={index === form.fields.length - 1}
                      onClick={() => setForm(moveField(form, index, 1))}
                    >
                      <Icon name="down" size={16} />
                    </button>
                    <button
                      type="button"
                      className="icon-button icon-button-danger"
                      aria-label={`הסרת שדה ${index + 1}`}
                      title="הסרת השדה (תחקירים קיימים לא יושפעו)"
                      onClick={() => setForm(removeField(form, index))}
                    >
                      <Icon name="trash" size={16} />
                    </button>
                  </div>
                </li>
              );
            })}
          </ol>
          <Button icon="plus" onClick={() => setForm(addField(form))}>
            הוספת שדה
          </Button>
          <p className="field-hint">
            שדה מושבת לא יוצג בתחקירים חדשים ונשמר בהגדרות. הסרת שדה מוחקת אותו מההגדרות בלבד: תחקירים שכבר נוצרו
            שומרים את הנתונים שלהם.
          </p>
        </div>
      </div>
      <footer className="editor-footer">
        <Button type="submit" variant="primary" busy={busy} disabled={busy}>
          {section ? 'שמירת הסעיף' : 'יצירת הסעיף'}
        </Button>
      </footer>
    </form>
  );
}
