import { useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { LaunchHistoryEntry } from '@/types/library';
import { normalizeSerializedGameProfile, type SerializedGameProfile } from '@/types/profile';

import { formatRelativeTime } from '../utils/format';
import { nextGameDetailsRequestId, useGameDetailsRequestCounter } from './useGameDetailsRequestGuards';

export interface ProfileCardMeta {
  protonLabel: string | null;
  lastUsedLabel: string | null;
}

export interface UseProfileCardMetaResult {
  metaByProfileName: Record<string, ProfileCardMeta>;
  loading: boolean;
}

const EMPTY_PROFILE_CARD_META: ProfileCardMeta = {
  protonLabel: null,
  lastUsedLabel: null,
};

function basename(path: string | null | undefined): string | null {
  const trimmed = path?.trim();
  if (!trimmed) {
    return null;
  }

  const normalized = trimmed.replace(/\\/g, '/').replace(/\/+$/, '');
  const label = normalized.split('/').pop()?.trim();
  return label || null;
}

async function loadProfileCardMeta(name: string): Promise<[string, ProfileCardMeta]> {
  try {
    const [serializedProfile, entries] = await Promise.all([
      callCommand<SerializedGameProfile>('profile_load', { name }),
      callCommand<LaunchHistoryEntry[]>('list_launch_history_for_profile', { profileName: name, limit: 1 }),
    ]);
    const profile = normalizeSerializedGameProfile(serializedProfile);

    return [
      name,
      {
        protonLabel: basename(profile.runtime?.proton_path || profile.steam.proton_path),
        lastUsedLabel: entries[0]?.started_at ? formatRelativeTime(entries[0].started_at) : null,
      },
    ];
  } catch {
    return [name, EMPTY_PROFILE_CARD_META];
  }
}

export function useProfileCardMeta(profileNames: string[]): UseProfileCardMetaResult {
  const requestCounter = useGameDetailsRequestCounter();
  const profileNamesKey = profileNames.join('\u0000');
  const [metaByProfileName, setMetaByProfileName] = useState<Record<string, ProfileCardMeta>>({});
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const names = profileNamesKey ? profileNamesKey.split('\u0000').filter((name) => name.length > 0) : [];
    const requestId = nextGameDetailsRequestId(requestCounter);

    if (names.length === 0) {
      setMetaByProfileName({});
      setLoading(false);
      return;
    }

    setMetaByProfileName({});
    setLoading(true);

    void Promise.all(names.map(loadProfileCardMeta)).then((entries) => {
      if (requestId !== requestCounter.current) {
        return;
      }

      setMetaByProfileName(Object.fromEntries(entries));
      setLoading(false);
    });
  }, [profileNamesKey, requestCounter]);

  return { metaByProfileName, loading };
}
