import { useGameMetadata } from '../../hooks/useGameMetadata';
import type { SteamGenre } from '../../types/game-metadata';

export interface GameMetadataBarProps {
  steamAppId: string | undefined;
}

export function GameMetadataBar({ steamAppId }: GameMetadataBarProps) {
  const { result } = useGameMetadata(steamAppId);

  if (result.state === 'idle' || result.state === 'unavailable') {
    return null;
  }

  const name = result.app_details?.name ?? null;
  const genres = result.app_details?.genres ?? [];
  const isStale = result.state === 'stale';

  return (
    <div className="crosshook-game-metadata-bar">
      {name ? <span className="crosshook-game-metadata-bar__name">{name}</span> : null}
      {genres.length > 0 || isStale ? (
        <div className="crosshook-game-metadata-bar__genres">
          {genres.map((genre: SteamGenre) => (
            <span key={genre.id} className="crosshook-game-metadata-bar__genre">
              {genre.description}
            </span>
          ))}
          {isStale ? (
            <span className="crosshook-game-metadata-bar__genre crosshook-game-metadata-bar__genre--stale">Cached</span>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

export default GameMetadataBar;
