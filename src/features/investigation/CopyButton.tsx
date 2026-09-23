import { useEffect, useState } from 'react';

import { Icon } from '../../ui/Icon';

/**
 * Copies a value to the clipboard. Links are copied rather than opened, so the
 * application never navigates to or fetches user-provided URLs.
 */
export function CopyButton({ value, label }: { value: string; label: string }) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle');

  useEffect(() => {
    if (state === 'idle') return;
    const timer = window.setTimeout(() => setState('idle'), 2000);
    return () => window.clearTimeout(timer);
  }, [state]);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setState('copied');
    } catch {
      setState('failed');
    }
  }

  const text = state === 'copied' ? 'הועתק' : state === 'failed' ? 'ההעתקה נכשלה' : label;
  return (
    <button type="button" className="copy-button" onClick={copy} aria-live="polite">
      <Icon name={state === 'copied' ? 'check' : 'copy'} size={16} />
      {text}
    </button>
  );
}
