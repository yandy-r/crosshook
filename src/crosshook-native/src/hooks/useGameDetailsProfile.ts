import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

import type { GameProfile, SerializedGameProfile } from '../types';
import { normalizeSerializedGameProfile } from '../types';
import { nextGameDetailsRequestId, useGameDetailsRequestCounter } from './useGameDetailsRequestGuards';

export type GameDetailsProfileLoadState = 'idle' | 'loading' | 'ready' | 'error';

export interface UseGameDetailsProfileResult {
  loadState: GameDetailsProfileLoadState;
  profile: GameProfile | null;
  errorMessage: string | null;
}

function formatLoadError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === 'string') {
    return err;
  }
  return String(err);
}

export function useGameDetailsProfile(profileName: string | null, open: boolean): UseGameDetailsProfileResult {
  const requestCounter = useGameDetailsRequestCounter();
  const [loadState, setLoadState] = useState<GameDetailsProfileLoadState>('idle');
  const [profile, setProfile] = useState<GameProfile | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !profileName?.trim()) {
      nextGameDetailsRequestId(requestCounter);
      setLoadState('idle');
      setProfile(null);
      setErrorMessage(null);
      return;
    }

    const trimmed = profileName.trim();
    const requestId = nextGameDetailsRequestId(requestCounter);
    setLoadState('loading');
    setProfile(null);
    setErrorMessage(null);

    void invoke<SerializedGameProfile>('profile_load', { name: trimmed })
      .then((data) => {
        if (requestId !== requestCounter.current) {
          return;
        }
        setProfile(normalizeSerializedGameProfile(data));
        setLoadState('ready');
        setErrorMessage(null);
      })
      .catch((err: unknown) => {
        if (requestId !== requestCounter.current) {
          return;
        }
        setProfile(null);
        setLoadState('error');
        setErrorMessage(formatLoadError(err));
      });
  }, [open, profileName, requestCounter]);

  return { loadState, profile, errorMessage };
}
