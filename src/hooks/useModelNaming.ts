import { useCallback, useSyncExternalStore } from 'react';
import { modelNamingEnabled, setModelNaming, subscribeModelNaming } from '../services/naming/userNames';

/**
 * The relation-naming switch, shared by every workspace and remembered on this machine.
 * On: the on-device model suggests names. Off: names are built from the key columns, which is much faster.
 */
export function useModelNaming(): [boolean, (on: boolean) => void] {
  const on = useSyncExternalStore(subscribeModelNaming, modelNamingEnabled, () => true);
  const set = useCallback((value: boolean) => setModelNaming(value), []);
  return [on, set];
}
