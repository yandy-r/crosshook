import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import { resolveProtonUpProviderForVersion, useProtonUp } from '../../../hooks/useProtonUp';
import type { ProtonInstallOption } from '../../../types/proton';
import type { ProtonUpSuggestion } from '../../../types/protonup';
import type { CommunityIndexedProfileRow } from './constants';
import { sortProtonInstalls } from './utils';

interface UseProfilesPageProtonArgs {
  effectiveSteamClientInstallPath: string;
  gameName: string;
  selectedProfile: string;
}

export function useProfilesPageProton({
  effectiveSteamClientInstallPath,
  gameName,
  selectedProfile,
}: UseProfilesPageProtonArgs) {
  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>([]);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);
  const protonUp = useProtonUp({
    steamClientInstallPath: effectiveSteamClientInstallPath,
  });
  const { getSuggestion, installVersion } = protonUp;
  const [suggestion, setSuggestion] = useState<ProtonUpSuggestion | null>(null);
  const [suggestionDismissed, setSuggestionDismissed] = useState(false);
  const [suggestionInstallError, setSuggestionInstallError] = useState<string | null>(null);

  const reloadProtonInstalls = useCallback(async () => {
    const installs = await callCommand<ProtonInstallOption[]>('list_proton_installs', {
      steamClientInstallPath:
        effectiveSteamClientInstallPath.trim().length > 0 ? effectiveSteamClientInstallPath : undefined,
    });
    setProtonInstalls(sortProtonInstalls(installs));
    setProtonInstallsError(null);
  }, [effectiveSteamClientInstallPath]);

  useEffect(() => {
    let active = true;

    async function loadProtonInstalls() {
      try {
        const installs = await callCommand<ProtonInstallOption[]>('list_proton_installs', {
          steamClientInstallPath:
            effectiveSteamClientInstallPath.trim().length > 0 ? effectiveSteamClientInstallPath : undefined,
        });

        if (!active) {
          return;
        }

        setProtonInstalls(sortProtonInstalls(installs));
        setProtonInstallsError(null);
      } catch (loadError) {
        if (!active) {
          return;
        }

        setProtonInstalls([]);
        setProtonInstallsError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    }

    void loadProtonInstalls();

    return () => {
      active = false;
    };
  }, [effectiveSteamClientInstallPath]);

  useEffect(() => {
    setSuggestionDismissed(false);
    setSuggestion(null);
    setSuggestionInstallError(null);

    const normalizedName = gameName.trim();
    if (!normalizedName || !selectedProfile) {
      return;
    }

    let active = true;

    async function fetchSuggestion() {
      try {
        const rows = await callCommand<CommunityIndexedProfileRow[]>('community_list_indexed_profiles');
        if (!active) {
          return;
        }

        const normalizedGame = normalizedName.toLowerCase();
        const match = rows.find(
          (row) =>
            typeof row.game_name === 'string' &&
            row.game_name.trim().toLowerCase() === normalizedGame &&
            typeof row.proton_version === 'string' &&
            row.proton_version.trim().length > 0
        );

        if (!match?.proton_version) {
          return;
        }

        const result = await getSuggestion(match.proton_version);
        if (active) {
          setSuggestion(result);
        }
      } catch {
        // Advisory-only: silently ignore errors; no suggestion shown on failure
      }
    }

    void fetchSuggestion();

    return () => {
      active = false;
    };
  }, [gameName, getSuggestion, selectedProfile]);

  const handleInstallSuggestedVersion = useCallback(async () => {
    if (!suggestion?.recommended_version) {
      return;
    }

    const recommendedVersion = suggestion.recommended_version;
    const targetRoot = effectiveSteamClientInstallPath ? `${effectiveSteamClientInstallPath}/compatibilitytools.d` : '';
    setSuggestionInstallError(null);

    const provider = await resolveProtonUpProviderForVersion(recommendedVersion);
    const result = await installVersion({
      provider,
      version: recommendedVersion,
      target_root: targetRoot,
    });
    if (!result.success) {
      setSuggestionInstallError(result.error_message ?? result.error_kind ?? 'Install failed');
      return;
    }

    await reloadProtonInstalls();
    setSuggestionDismissed(true);
  }, [effectiveSteamClientInstallPath, installVersion, reloadProtonInstalls, suggestion]);

  return {
    handleInstallSuggestedVersion,
    protonInstalls,
    protonInstallsError,
    protonUp,
    suggestion,
    suggestionDismissed,
    suggestionInstallError,
    setSuggestionDismissed,
  };
}
