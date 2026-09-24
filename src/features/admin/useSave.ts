import { useState } from 'react';

import { ApiError, errorMessage, fieldErrors } from '../../api/errors';

/**
 * Runs an admin save and keeps its outcome: field errors (by backend field
 * name), a general error, or a success notice.
 */
export function useSave() {
  const [busy, setBusy] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function save(work: () => Promise<unknown>, success: string): Promise<boolean> {
    setBusy(true);
    setFailure(null);
    setNotice(null);
    try {
      await work();
      setErrors({});
      setNotice(success);
      return true;
    } catch (caught) {
      const byField = fieldErrors(caught);
      setErrors(byField);
      const isValidation = caught instanceof ApiError && caught.appError.kind === 'validation';
      setFailure(isValidation ? 'יש לתקן את השדות המסומנים.' : errorMessage(caught));
      return false;
    } finally {
      setBusy(false);
    }
  }

  const clear = () => {
    setErrors({});
    setFailure(null);
    setNotice(null);
  };

  return { busy, errors, failure, notice, save, clear };
}
