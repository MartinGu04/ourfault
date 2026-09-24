import { useState } from 'react';

import { api } from '../../api/client';
import type { Station } from '../../api/types';
import { Badge, Banner, Button } from '../../ui/controls';
import { useSave } from './useSave';

/** The central list of satellite stations: create, rename, (de)activate. */
export function StationsPanel({ stations, onChanged }: { stations: Station[]; onChanged: () => void }) {
  const [newName, setNewName] = useState('');
  const { busy, errors, failure, notice, save } = useSave();

  async function add() {
    if (await save(() => api.adminSaveStation({ id: null, name: newName }), 'התחנה נוספה.')) {
      setNewName('');
      onChanged();
    }
  }

  return (
    <div className="editor">
      <header className="editor-header">
        <div className="editor-title">
          <h2>תחנות לווייניות</h2>
        </div>
      </header>
      <div className="editor-body">
        <p className="field-hint">
          שדות מסוג „תחנה לוויינית” בסעיפים הטכניים מציעים את התחנות הפעילות. תחנות אינן נמחקות; תחנה מושבתת נשמרת
          בתחקירים קיימים.
        </p>
        {notice && !failure && <Banner tone="success">{notice}</Banner>}
        {failure && <Banner tone="error">{failure}</Banner>}
        <ul className="station-list">
          {stations.map((station) => (
            <StationRow
              key={station.id}
              station={station}
              busy={busy}
              onRename={async (name) => {
                if (await save(() => api.adminSaveStation({ id: station.id, name }), 'שם התחנה עודכן.')) onChanged();
              }}
              onToggle={async () => {
                const done = station.active ? 'התחנה הושבתה.' : 'התחנה הופעלה.';
                if (await save(() => api.adminSetStationActive(station.id, !station.active), done)) onChanged();
              }}
            />
          ))}
        </ul>
        <form
          className="inline-form"
          onSubmit={(event) => {
            event.preventDefault();
            void add();
          }}
        >
          <input
            className="input"
            aria-label="שם תחנה חדשה"
            placeholder="שם תחנה חדשה"
            value={newName}
            maxLength={80}
            aria-invalid={Boolean(errors.name) || undefined}
            onChange={(event) => setNewName(event.target.value)}
          />
          <Button type="submit" icon="plus" busy={busy} disabled={!newName.trim()}>
            הוספת תחנה
          </Button>
        </form>
        {errors.name && <p className="field-error">{errors.name}</p>}
      </div>
    </div>
  );
}

interface RowProps {
  station: Station;
  busy: boolean;
  onRename: (name: string) => Promise<void>;
  onToggle: () => Promise<void>;
}

function StationRow({ station, busy, onRename, onToggle }: RowProps) {
  const [name, setName] = useState(station.name);
  const dirty = name.trim() !== station.name;
  return (
    <li className="station-row">
      <input
        className="input"
        aria-label={`שם התחנה ${station.name}`}
        value={name}
        maxLength={80}
        onChange={(event) => setName(event.target.value)}
      />
      <Badge tone={station.active ? 'success' : 'muted'}>{station.active ? 'פעילה' : 'מושבתת'}</Badge>
      {dirty && (
        <Button variant="primary" disabled={busy || !name.trim()} onClick={() => void onRename(name)}>
          שמירת שם
        </Button>
      )}
      <Button variant="subtle" disabled={busy} onClick={() => void onToggle()}>
        {station.active ? 'השבתה' : 'הפעלה'}
      </Button>
    </li>
  );
}
