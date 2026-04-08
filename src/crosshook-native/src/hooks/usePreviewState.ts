import { useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { LaunchPreview, LaunchRequest } from '../types';

export function usePreviewState() {
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<LaunchPreview | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function requestPreview(request: LaunchRequest) {
    setLoading(true);
    setPreview(null);
    setError(null);

    try {
      const result = await callCommand<LaunchPreview>('preview_launch', { request });
      setPreview(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  function clearPreview() {
    setLoading(false);
    setPreview(null);
    setError(null);
  }

  return { loading, preview, error, requestPreview, clearPreview };
}
