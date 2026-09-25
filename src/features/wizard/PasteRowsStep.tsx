// Step 3 (event chronology): the operator copies the relevant rows in Excel and pastes them here,
// then reviews them. Nothing is inferred: every row shown is a row that was
// pasted, and rows change only when the operator edits or removes them.

import { useEffect, useRef, useState, type Dispatch } from 'react';

import demoPaste from '../../../demo/demo-paste.txt?raw';
import { fieldMessage } from '../../api/errors';
import type { LogRow } from '../../api/types';
import { rowsLabel } from '../../lib/format';
import { Banner, Button, Ltr } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { readClipboardText, type ClipboardRead } from './clipboard';
import { InlineMessage } from './InlineAdvisory';
import { errorFor, pasteFeedback, type ReviewRow, type WizardAction, type WizardState } from './wizardState';

interface Props {
  state: WizardState;
  dispatch: Dispatch<WizardAction>;
  /** Parses pasted text through the backend and adds the rows. */
  onPasteText: (read: ClipboardRead) => void;
  parsing: boolean;
}

export function PasteRowsStep({ state, dispatch, onPasteText, parsing }: Props) {
  useClipboardPaste(onPasteText);
  const rowsError = errorFor(state, 'rows');

  if (state.rows.length === 0) {
    return (
      <div className="paste-panel">
        <div className="paste-icon">
          <Icon name="clipboard" size={30} />
        </div>
        <h2>הדבקת שורות מיומן המבצעים</h2>
        <ol className="paste-steps">
          <li>
            <span>1</span>סמנו ב-Excel את השורות הרלוונטיות לתחקיר
          </li>
          <li>
            <span>2</span>העתיקו אותן <kbd>Ctrl+C</kbd>
          </li>
          <li>
            <span>3</span>הדביקו כאן <kbd>Ctrl+V</kbd>
          </li>
        </ol>
        {rowsError && <Banner tone="error">{fieldMessage(rowsError)}</Banner>}
        <PasteTarget label="לחצו כאן והדביקו את השורות שהעתקתם" parsing={parsing} autoFocus />
        <p className="paste-note">
          העמודות נקראות לפי סדר היומן: שעה, ממי, למי, תוכן. אם הועתקה גם שורת הכותרות, היא מושמטת.
        </p>
        <Button variant="subtle" disabled={parsing} onClick={() => onPasteText({ kind: 'text', text: demoPaste })}>
          הדבקת שורות לדוגמה
        </Button>
      </div>
    );
  }

  return (
    <div className="rows-step">
      <div className="review-bar">
        <p className="review-count">{rowsLabel(state.rows.length)} לתחקיר</p>
        <PasteTarget label="הדבקת שורות נוספות (Ctrl+V)" parsing={parsing} compact />
        <Button variant="subtle" icon="trash" disabled={parsing} onClick={() => dispatch({ type: 'rowsCleared' })}>
          ניקוי הכל
        </Button>
      </div>

      {rowsError && <Banner tone="error">{fieldMessage(rowsError)}</Banner>}
      {state.lastPaste && (
        <InlineMessage tone="success">
          {pasteFeedback(state.lastPaste, state.rows.length)} בדקו את השורות, וערכו או הסירו שורות לפי הצורך.
        </InlineMessage>
      )}

      <div className="table-wrap">
        <table className="data-table review-table">
          <thead>
            <tr>
              <th scope="col" className="col-index">
                #
              </th>
              <th scope="col" className="col-time">
                שעה
              </th>
              <th scope="col" className="col-party">
                ממי
              </th>
              <th scope="col" className="col-party">
                למי
              </th>
              <th scope="col">תוכן</th>
              <th scope="col" className="col-actions">
                <span className="visually-hidden">פעולות</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {state.rows.map((row, index) => (
              <ReviewTableRow key={row.key} row={row} index={index + 1} dispatch={dispatch} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/**
 * Accepts pastes anywhere on the step, except inside the fields used to edit
 * a row (where paste keeps its normal meaning). Only the plain-text flavour
 * of the clipboard is used (never HTML, images or files), and only when the
 * operator pastes.
 */
function useClipboardPaste(onPasteText: (read: ClipboardRead) => void) {
  const handler = useRef(onPasteText);
  handler.current = onPasteText;

  useEffect(() => {
    function onPaste(event: ClipboardEvent) {
      const target = event.target instanceof Element ? event.target : null;
      const inEditor = target?.closest('input, textarea, select') && !target.closest('[data-paste-target]');
      if (inEditor) return;
      event.preventDefault();
      handler.current(readClipboardText(event.clipboardData));
    }
    window.addEventListener('paste', onPaste);
    return () => window.removeEventListener('paste', onPaste);
  }, []);
}

/** A focusable box to paste into. Typing is ignored; only pastes count. */
function PasteTarget(props: { label: string; parsing: boolean; compact?: boolean; autoFocus?: boolean }) {
  return (
    <textarea
      data-paste-target
      className={props.compact ? 'paste-target paste-target-compact' : 'paste-target'}
      aria-label={props.label}
      placeholder={props.parsing ? 'קורא את השורות…' : props.label}
      value=""
      onChange={() => undefined}
      rows={props.compact ? 1 : 3}
      spellCheck={false}
      autoFocus={props.autoFocus}
    />
  );
}

function ReviewTableRow({ row, index, dispatch }: { row: ReviewRow; index: number; dispatch: Dispatch<WizardAction> }) {
  const [editing, setEditing] = useState(false);

  if (editing) {
    return (
      <tr className="is-editing">
        <td className="col-index">{index}</td>
        <td colSpan={5}>
          <RowEditor
            row={row}
            onCancel={() => setEditing(false)}
            onSave={(updated) => {
              dispatch({ type: 'rowUpdated', key: row.key, row: updated });
              setEditing(false);
            }}
          />
        </td>
      </tr>
    );
  }

  return (
    <tr>
      <td className="col-index">{index}</td>
      <td className="col-time">
        <Ltr>{row.time}</Ltr>
      </td>
      <td>{row.from}</td>
      <td>{row.to}</td>
      <td className="cell-text">{row.description}</td>
      <td className="col-actions">
        <button type="button" className="icon-button" aria-label={`עריכת שורה ${index}`} title="עריכה" onClick={() => setEditing(true)}>
          <Icon name="edit" size={16} />
        </button>
        <button
          type="button"
          className="icon-button icon-button-danger"
          aria-label={`הסרת שורה ${index}`}
          title="הסרה"
          onClick={() => dispatch({ type: 'rowRemoved', key: row.key })}
        >
          <Icon name="trash" size={16} />
        </button>
      </td>
    </tr>
  );
}

function RowEditor({ row, onSave, onCancel }: { row: LogRow; onSave: (row: LogRow) => void; onCancel: () => void }) {
  const [draft, setDraft] = useState<LogRow>(row);
  const field = (key: keyof LogRow, label: string) => (
    <label className="row-editor-field">
      <span>{label}</span>
      <input
        className="input"
        value={draft[key]}
        maxLength={4000}
        onChange={(event) => setDraft({ ...draft, [key]: event.target.value })}
      />
    </label>
  );
  return (
    <form
      className="row-editor"
      onSubmit={(event) => {
        event.preventDefault();
        onSave(draft);
      }}
    >
      <div className="row-editor-grid">
        {field('time', 'שעה')}
        {field('from', 'ממי')}
        {field('to', 'למי')}
      </div>
      <label className="row-editor-field">
        <span>תוכן</span>
        <textarea
          className="input textarea"
          rows={3}
          value={draft.description}
          maxLength={4000}
          onChange={(event) => setDraft({ ...draft, description: event.target.value })}
        />
      </label>
      <div className="row-editor-actions">
        <Button variant="subtle" onClick={onCancel}>
          ביטול
        </Button>
        <Button type="submit" variant="primary">
          שמירת השורה
        </Button>
      </div>
    </form>
  );
}
