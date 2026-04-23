import { useCallback, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { LaunchPreview, LaunchRequest } from '../types';

export function usePreviewState() {
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<LaunchPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Which launch path the last preview request targeted (for "Launch from preview"). */
  const [previewTarget, setPreviewTarget] = useState<'game' | 'trainer' | null>(null);
  const previewRequestSeq = useRef(0);

  const requestPreview = useCallback(async (request: LaunchRequest) => {
    const seq = ++previewRequestSeq.current;
    setLoading(true);
    setPreview(null);
    setError(null);
    const target =
      request.preview_target ?? (request.launch_trainer_only ? 'trainer' : request.launch_game_only ? 'game' : null);
    setPreviewTarget(target);

    try {
      const result = await callCommand<LaunchPreview>('preview_launch', { request });
      if (seq !== previewRequestSeq.current) return;
      setPreview(result);
    } catch (err) {
      if (seq !== previewRequestSeq.current) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (seq === previewRequestSeq.current) {
        setLoading(false);
      }
    }
  }, []);

  const clearPreview = useCallback(() => {
    setLoading(false);
    setPreview(null);
    setError(null);
    setPreviewTarget(null);
  }, []);

  return { loading, preview, error, requestPreview, clearPreview, previewTarget };
}
