import { useState } from 'react';

import { api } from '../../api/client';
import type { Configuration, MailTemplate, PublicationSettings } from '../../api/types';
import { Banner, Button, Field, Ltr } from '../../ui/controls';
import { useSave } from './useSave';

const PLACEHOLDERS: { name: string; meaning: string }[] = [
  { name: '{{ACTIVITY_NAME}}', meaning: 'שם המשימה / הפעילות' },
  { name: '{{SYSTEMS}}', meaning: 'המערכות המופעלות' },
  { name: '{{INVESTIGATION_NUMBER}}', meaning: 'מספר התחקיר' },
  { name: '{{INVESTIGATION_LINK}}', meaning: 'קישור לתחקיר (מוצג כמילת הקישור)' },
];

export function PublishingPanel({ configuration, onChanged }: { configuration: Configuration; onChanged: () => void }) {
  return (
    <div className="publishing">
      <MailTemplateEditor template={configuration.mail} onChanged={onChanged} />
      <PublicationEditor settings={configuration.publication} onChanged={onChanged} />
    </div>
  );
}

function MailTemplateEditor({ template, onChanged }: { template: MailTemplate; onChanged: () => void }) {
  const [values, setValues] = useState(template);
  const { busy, errors, failure, notice, save } = useSave();

  return (
    <form
      className="editor"
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        void save(() => api.adminSaveMailTemplate(values), 'תבנית ההודעה נשמרה.').then((ok) => ok && onChanged());
      }}
    >
      <header className="editor-header">
        <div className="editor-title">
          <h2>תבנית הודעת ההפצה</h2>
        </div>
      </header>
      <div className="editor-body">
        <p className="field-hint">
          תבנית אחת לכל התחקירים. הנמענים הם כל אנשי הקשר של כל המערכות בתחקיר, ללא כפילויות. ההודעה נשלחת מהמשתמש
          המחובר ב-Outlook, עם החתימה שלו.
        </p>
        {notice && !failure && <Banner tone="success">{notice}</Banner>}
        {failure && <Banner tone="error">{failure}</Banner>}
        <Field label="נושא" error={errors.subject}>
          {({ id, describedBy, invalid }) => (
            <input
              id={id}
              className="input"
              value={values.subject}
              maxLength={200}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => setValues({ ...values, subject: event.target.value })}
            />
          )}
        </Field>
        <Field label="גוף ההודעה" error={errors.body}>
          {({ id, describedBy, invalid }) => (
            <textarea
              id={id}
              className="input textarea"
              rows={9}
              value={values.body}
              maxLength={4000}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => setValues({ ...values, body: event.target.value })}
            />
          )}
        </Field>
        <Field label="טקסט הקישור לתחקיר" error={errors.linkText} hint="המילה שתוצג כקישור במקום {{INVESTIGATION_LINK}}.">
          {({ id, describedBy, invalid }) => (
            <input
              id={id}
              className="input input-short"
              value={values.linkText}
              maxLength={40}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => setValues({ ...values, linkText: event.target.value })}
            />
          )}
        </Field>
        <dl className="placeholder-list">
          {PLACEHOLDERS.map((placeholder) => (
            <div key={placeholder.name}>
              <dt>
                <Ltr>{placeholder.name}</Ltr>
              </dt>
              <dd>{placeholder.meaning}</dd>
            </div>
          ))}
        </dl>
      </div>
      <footer className="editor-footer">
        <Button type="submit" variant="primary" busy={busy}>
          שמירת התבנית
        </Button>
      </footer>
    </form>
  );
}

function PublicationEditor({ settings, onChanged }: { settings: PublicationSettings; onChanged: () => void }) {
  const [values, setValues] = useState(settings);
  const { busy, errors, failure, notice, save } = useSave();

  return (
    <form
      className="editor"
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        void save(() => api.adminSavePublication(values), 'יעד הפרסום נשמר.').then((ok) => ok && onChanged());
      }}
    >
      <header className="editor-header">
        <div className="editor-title">
          <h2>יעד פרסום ב-SharePoint</h2>
        </div>
      </header>
      <div className="editor-body">
        <p className="field-hint">
          כל התחקירים נוצרים ברשימה אחת, לכל שילוב של מערכות. בגרסה זו הפרסום הוא סימולציה מקומית ואין התחברות
          ל-SharePoint.
        </p>
        {notice && !failure && <Banner tone="success">{notice}</Banner>}
        {failure && <Banner tone="error">{failure}</Banner>}
        <div className="field-row">
          <Field label="כתובת האתר" error={errors.siteUrl}>
            {({ id, describedBy, invalid }) => (
              <input
                id={id}
                className="input input-ltr"
                dir="ltr"
                value={values.siteUrl}
                maxLength={2048}
                spellCheck={false}
                aria-describedby={describedBy}
                aria-invalid={invalid || undefined}
                onChange={(event) => setValues({ ...values, siteUrl: event.target.value })}
              />
            )}
          </Field>
          <Field label="רשימה" error={errors.list}>
            {({ id, describedBy, invalid }) => (
              <input
                id={id}
                className="input"
                value={values.list}
                maxLength={80}
                aria-describedby={describedBy}
                aria-invalid={invalid || undefined}
                onChange={(event) => setValues({ ...values, list: event.target.value })}
              />
            )}
          </Field>
        </div>
      </div>
      <footer className="editor-footer">
        <Button type="submit" variant="primary" busy={busy}>
          שמירת היעד
        </Button>
      </footer>
    </form>
  );
}
