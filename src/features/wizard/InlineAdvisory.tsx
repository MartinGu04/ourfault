// A compact finding shown next to the fields it is about, while editing.
// The full list stays on the review step.

import type { ReactNode } from 'react';

import { advisoryMessage } from '../../api/errors';
import type { Advisory } from '../../api/types';
import { Icon } from '../../ui/Icon';

export function InlineAdvisory({ advisory, action }: { advisory: Advisory; action?: ReactNode }) {
  return (
    <InlineMessage tone={advisory.severity} action={action}>
      {advisoryMessage(advisory)}
    </InlineMessage>
  );
}

export function InlineMessage({
  tone,
  action,
  children,
}: {
  tone: Advisory['severity'] | 'success';
  action?: ReactNode;
  children: ReactNode;
}) {
  const icon = tone === 'success' ? 'check' : tone === 'info' ? 'info' : 'alert';
  return (
    <div className={`inline-note inline-note-${tone}`} role={tone === 'error' ? 'alert' : 'status'}>
      <Icon name={icon} size={15} />
      <span className="inline-note-text">{children}</span>
      {action && <span className="inline-note-action">{action}</span>}
    </div>
  );
}
