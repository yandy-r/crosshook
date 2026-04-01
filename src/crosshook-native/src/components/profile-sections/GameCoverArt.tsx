import { useEffect, useState } from 'react';

import { useGameCoverArt } from '../../hooks/useGameCoverArt';

export interface GameCoverArtProps {
  steamAppId: string | undefined;
}

export function GameCoverArt({ steamAppId }: GameCoverArtProps) {
  const { coverArtUrl, loading } = useGameCoverArt(steamAppId);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [coverArtUrl]);

  if (!steamAppId) {
    return null;
  }

  if (loading) {
    return <div className="crosshook-profile-cover-art crosshook-skeleton" />;
  }

  if (!coverArtUrl || failed) {
    return null;
  }

  return (
    <img
      src={coverArtUrl}
      className="crosshook-profile-cover-art"
      alt="Game cover art"
      onError={() => setFailed(true)}
    />
  );
}

export default GameCoverArt;
