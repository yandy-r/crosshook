import { useGameCoverArt } from '../../hooks/useGameCoverArt';

export interface GameCoverArtProps {
  steamAppId: string | undefined;
}

export function GameCoverArt({ steamAppId }: GameCoverArtProps) {
  const { coverArtUrl, loading } = useGameCoverArt(steamAppId);

  if (!steamAppId) {
    return null;
  }

  if (loading) {
    return <div className="crosshook-profile-cover-art crosshook-skeleton" />;
  }

  if (!coverArtUrl) {
    return null;
  }

  return (
    <img
      src={coverArtUrl}
      className="crosshook-profile-cover-art"
      alt="Game cover art"
    />
  );
}

export default GameCoverArt;
