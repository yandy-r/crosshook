import { callCommand } from '@/lib/ipc';
import { useEffect, useMemo, useState } from 'react';

import type { TrainerTypeEntry } from '../types/offline';

export function useTrainerTypeCatalog() {
  const [catalog, setCatalog] = useState<TrainerTypeEntry[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void callCommand<TrainerTypeEntry[]>('get_trainer_type_catalog')
      .then((rows) => {
        if (!cancelled) {
          setCatalog(rows);
          setError(null);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setCatalog([]);
          setError('Could not load trainer type catalog.');
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const labels = useMemo(() => {
    if (catalog.length === 0) {
      return { unknown: 'Unknown' } as Record<string, string>;
    }
    const next: Record<string, string> = {};
    for (const entry of catalog) {
      next[entry.id] = entry.display_name;
    }
    return next;
  }, [catalog]);

  const selectOptions = useMemo(() => {
    if (catalog.length === 0) {
      return [{ value: 'unknown', label: 'Unknown' }];
    }
    return catalog.map((e) => ({ value: e.id, label: e.display_name }));
  }, [catalog]);

  return { catalog, labels, error, selectOptions };
}
