import type { InvestigationStatus } from '../../api/types';
import { INVESTIGATION_STATUS_LABELS } from '../../lib/labels';

const TONE: Record<InvestigationStatus, string> = {
  draft: 'badge badge-warning',
  completed: 'badge badge-accent',
  distributed: 'badge badge-success',
};

export function StatusBadge({ status }: { status: InvestigationStatus }) {
  return <span className={TONE[status]}>{INVESTIGATION_STATUS_LABELS[status]}</span>;
}
