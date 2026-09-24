import { useCallback, useEffect, useState } from 'react';

export type Resource<T> =
  | { status: 'loading' }
  | { status: 'ready'; data: T }
  | { status: 'error'; error: unknown };

/**
 * Loads data once on mount (and whenever `reload` is called). Results of
 * superseded loads are ignored.
 */
export function useResource<T>(load: () => Promise<T>): [Resource<T>, () => void] {
  const [state, setState] = useState<Resource<T>>({ status: 'loading' });
  const [generation, setGeneration] = useState(0);

  useEffect(() => {
    let current = true;
    setState((previous) => (previous.status === 'ready' ? previous : { status: 'loading' }));
    load().then(
      (data) => current && setState({ status: 'ready', data }),
      (error: unknown) => current && setState({ status: 'error', error }),
    );
    return () => {
      current = false;
    };
    // `load` is intentionally not a dependency: callers pass inline functions,
    // and reloading is explicit through `reload`.
  }, [generation]);

  const reload = useCallback(() => setGeneration((g) => g + 1), []);
  return [state, reload];
}
