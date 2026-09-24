import type { SaveStatus } from './autosave';
import { Icon } from '../../ui/Icon';

interface Props {
  status: SaveStatus;
  error: string | null;
  onRetry: () => void;
}

/** A quiet indicator: נשמר / שומר... / שגיאה בשמירה. */
export function AutosaveIndicator({ status, error, onRetry }: Props) {
  if (status === 'idle') {
    return <span className="autosave autosave-idle">הטיוטה תישמר אוטומטית</span>;
  }
  if (status === 'error') {
    return (
      <span className="autosave autosave-error" role="alert">
        <Icon name="alert" size={15} />
        שגיאה בשמירה
        {error && <span className="autosave-detail">{error}</span>}
        <button type="button" className="link-button" onClick={onRetry}>
          ניסיון חוזר
        </button>
      </span>
    );
  }
  const saving = status === 'saving' || status === 'pending';
  return (
    <span className={saving ? 'autosave autosave-saving' : 'autosave autosave-saved'} aria-live="polite">
      {saving ? <span className="spinner" /> : <Icon name="check" size={15} />}
      {saving ? 'שומר...' : 'נשמר'}
    </span>
  );
}
