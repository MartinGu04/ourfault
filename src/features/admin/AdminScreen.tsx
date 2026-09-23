import { useState } from 'react';

import type { Navigation } from '../../App';
import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import type { System } from '../../api/types';
import { useResource } from '../../lib/useResource';
import { Badge, Banner, Button, Spinner } from '../../ui/controls';
import { SystemEditor } from './SystemEditor';

type Selection = { mode: 'edit'; id: string } | { mode: 'new' };

export function AdminScreen({ navigation }: { navigation: Navigation }) {
  const [systems, reload] = useResource(api.adminListSystems);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const list = systems.status === 'ready' ? systems.data : [];
  const effective: Selection | null = selection ?? (list[0] ? { mode: 'edit', id: list[0].id } : null);
  const selected = effective?.mode === 'edit' ? list.find((system) => system.id === effective.id) : undefined;

  function select(next: Selection) {
    setNotice(null);
    setSelection(next);
  }

  function handleSaved(system: System, message: string) {
    setNotice(message);
    setSelection({ mode: 'edit', id: system.id });
    reload();
  }

  return (
    <div className="page admin">
      <div className="page-toolbar">
        <Button variant="subtle" icon="back" onClick={navigation.goHome}>
          דף הבית
        </Button>
      </div>
      <div className="page-heading">
        <div>
          <h1 className="page-title">ניהול מערכות</h1>
          <p className="page-subtitle">
            לכל מערכת מוגדרות תבנית תחקיר, רשימת תפוצה ויעד שמירה. מערכות אינן נמחקות: מערכת שהושבתה אינה מופיעה
            בתחקירים חדשים, והיסטוריית התחקירים שלה נשמרת.
          </p>
        </div>
        <Button variant="primary" icon="plus" onClick={() => select({ mode: 'new' })}>
          מערכת חדשה
        </Button>
      </div>

      {systems.status === 'loading' && <Spinner label="טוען מערכות" />}
      {systems.status === 'error' && <Banner tone="error">{errorMessage(systems.error)}</Banner>}
      {systems.status === 'ready' && (
        <div className="admin-layout">
          <nav className="system-list" aria-label="מערכות">
            {list.map((system) => (
              <button
                key={system.id}
                type="button"
                className="system-list-item"
                aria-current={selected?.id === system.id ? 'true' : undefined}
                onClick={() => select({ mode: 'edit', id: system.id })}
              >
                <span className="system-list-name">{system.name}</span>
                <Badge tone={system.active ? 'success' : 'muted'}>{system.active ? 'פעילה' : 'מושבתת'}</Badge>
                <span className="system-list-meta">{system.template.name}</span>
              </button>
            ))}
            {effective?.mode === 'new' && (
              <div className="system-list-item" aria-current="true">
                <span className="system-list-name">מערכת חדשה</span>
              </div>
            )}
          </nav>

          {effective?.mode === 'new' && (
            <SystemEditor
              key="new"
              system={null}
              notice={notice}
              onSaved={(system) => handleSaved(system, 'המערכת נוצרה.')}
              onCancel={() => select(list[0] ? { mode: 'edit', id: list[0].id } : { mode: 'new' })}
            />
          )}
          {selected && (
            <SystemEditor
              key={selected.id}
              system={selected}
              notice={notice}
              onSaved={(system) => handleSaved(system, 'השינויים נשמרו.')}
              onActiveChanged={(system) =>
                handleSaved(system, system.active ? 'המערכת הופעלה.' : 'המערכת הושבתה. היא לא תופיע בתחקירים חדשים.')
              }
            />
          )}
        </div>
      )}
    </div>
  );
}
