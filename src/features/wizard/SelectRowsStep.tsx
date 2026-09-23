import type { Dispatch } from 'react';

import { fieldMessage } from '../../api/errors';
import type { OperationsLogRow } from '../../api/types';
import { rowsLabel } from '../../lib/format';
import { Banner, Button, Ltr } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { errorFor, type WizardAction, type WizardState } from './wizardState';

interface Props {
  state: WizardState;
  dispatch: Dispatch<WizardAction>;
  onImport: () => void;
  onImportDemo: () => void;
  busy: 'import' | 'demo' | null;
}

export function SelectRowsStep({ state, dispatch, onImport, onImportDemo, busy }: Props) {
  const rowsError = errorFor(state, 'rows');

  if (!state.log) {
    return (
      <div className="import-panel">
        <div className="import-icon">
          <Icon name="sheet" size={32} />
        </div>
        <h2>ייבוא יומן המבצעים</h2>
        <p>
          בחרו את קובץ ה-Excel של יומן המבצעים. השורות יוצגו בטבלה, ושורות שסומנו בצבע בקובץ ייבחרו מראש.
        </p>
        {rowsError && <Banner tone="error">{fieldMessage(rowsError)}</Banner>}
        <div className="import-actions">
          <Button variant="primary" icon="folder" busy={busy === 'import'} disabled={busy !== null} onClick={onImport}>
            בחירת קובץ Excel…
          </Button>
          <Button busy={busy === 'demo'} disabled={busy !== null} onClick={onImportDemo}>
            שימוש בקובץ הדגמה
          </Button>
        </div>
        <p className="import-note">קובצי xlsx בלבד. הקובץ נקרא מקומית ואינו נשלח לשום מקום.</p>
      </div>
    );
  }

  const { log } = state;
  const highlightedCount = log.rows.filter((row) => row.highlighted).length;
  const allSelected = state.selectedRowIds.size === log.rows.length;
  const someSelected = state.selectedRowIds.size > 0 && !allSelected;

  return (
    <div className="rows-step">
      <div className="file-bar">
        <div className="file-info">
          <Icon name="sheet" />
          <Ltr className="file-name">{log.sourceFileName}</Ltr>
          <span className="file-meta">
            גיליון: {log.sheetName} · {rowsLabel(log.rows.length)}
          </span>
        </div>
        <Button variant="subtle" icon="refresh" busy={busy === 'import'} disabled={busy !== null} onClick={onImport}>
          החלפת קובץ
        </Button>
      </div>

      {rowsError && <Banner tone="error">{fieldMessage(rowsError)}</Banner>}
      {highlightedCount > 0 && (
        <Banner>
          <span className="mark-swatch" aria-hidden="true" /> {rowsLabel(highlightedCount)} שסומנו בצבע בקובץ נבחרו מראש.
          אפשר להוסיף או להסיר שורות לפי הצורך.
        </Banner>
      )}
      {!log.highlightDetectionAvailable && (
        <Banner>לא ניתן היה לזהות סימוני צבע בקובץ. יש לבחור את השורות הרלוונטיות ידנית.</Banner>
      )}

      <div className="table-wrap">
        <table className="data-table selectable">
          <thead>
            <tr>
              <th scope="col" className="col-check">
                <input
                  type="checkbox"
                  aria-label={allSelected ? 'ניקוי הבחירה' : 'בחירת כל השורות'}
                  checked={allSelected}
                  ref={(input) => {
                    if (input) input.indeterminate = someSelected;
                  }}
                  onChange={() => dispatch({ type: 'allRowsSet', selected: !allSelected })}
                />
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
              <th scope="col" className="col-type">
                סוג אירוע
              </th>
            </tr>
          </thead>
          <tbody>
            {log.rows.map((row) => (
              <LogRow
                key={row.id}
                row={row}
                selected={state.selectedRowIds.has(row.id)}
                onToggle={() => dispatch({ type: 'rowToggled', rowId: row.id })}
              />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function LogRow({ row, selected, onToggle }: { row: OperationsLogRow; selected: boolean; onToggle: () => void }) {
  const classes = [selected ? 'is-selected' : '', row.highlighted ? 'is-marked' : ''].filter(Boolean).join(' ');
  return (
    <tr className={classes || undefined} onClick={onToggle}>
      <td className="col-check">
        <input
          type="checkbox"
          checked={selected}
          aria-label={`בחירת שורה ${row.id}: ${row.time} ${row.description}`}
          onClick={(event) => event.stopPropagation()}
          onChange={onToggle}
        />
      </td>
      <td className="col-time">
        <Ltr>{row.time}</Ltr>
      </td>
      <td>{row.from}</td>
      <td>{row.to}</td>
      <td className="cell-text">
        {row.highlighted && <span className="mark-swatch" role="img" aria-label="מסומן בצבע בקובץ" title="מסומן בצבע בקובץ המקורי" />}
        {row.description}
      </td>
      <td className="col-type">{row.eventType && <span className="tag">{row.eventType}</span>}</td>
    </tr>
  );
}
