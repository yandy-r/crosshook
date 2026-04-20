export function isSteamDeckRuntime(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }

  const globalCandidate = window as Window &
    typeof globalThis & {
      SteamDeck?: string | number | boolean;
      STEAM_DECK?: string | number | boolean;
    };

  const flagValues = [
    globalCandidate.SteamDeck,
    globalCandidate.STEAM_DECK,
    (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.SteamDeck,
    (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.STEAM_DECK,
    (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.VITE_STEAM_DECK,
  ];

  if (
    flagValues.some((value) => value === true || value === 1 || value === '1' || value === 'true' || value === 'TRUE')
  ) {
    return true;
  }

  const coarsePointer = window.matchMedia?.('(pointer: coarse)').matches ?? false;
  const handheldViewport = window.matchMedia?.('(max-width: 1280px) and (max-height: 800px)').matches ?? false;
  const userAgent = window.navigator.userAgent.toLowerCase();

  return coarsePointer && handheldViewport && userAgent.includes('steam');
}
