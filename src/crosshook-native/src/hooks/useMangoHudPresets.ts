import { useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

export interface MangoHudPreset {
  id: string;
  label: string;
  description: string;
  fps_limit?: number;
  gpu_stats: boolean;
  cpu_stats: boolean;
  ram: boolean;
  frametime: boolean;
  battery: boolean;
  watt: boolean;
  position?: string;
}

export interface UseMangoHudPresetsResult {
  presets: MangoHudPreset[];
  loading: boolean;
  error: string | null;
}

/** React hook that fetches MangoHud presets from the backend. */
export function useMangoHudPresets(): UseMangoHudPresetsResult {
  const [presets, setPresets] = useState<MangoHudPreset[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void callCommand<MangoHudPreset[]>('get_mangohud_presets')
      .then((data) => {
        if (!cancelled) {
          setPresets(data);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(String(err));
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return { presets, loading, error };
}
