import { useEffect, useState } from 'react';

import { useGameCoverArt } from '../../hooks/useGameCoverArt';

export interface GameCoverArtProps {
  steamAppId: string | undefined;
  customCoverArtPath?: string;
}

export function GameCoverArt({ steamAppId, customCoverArtPath }: GameCoverArtProps) {
  const { coverArtUrl, loading } = useGameCoverArt(steamAppId, customCoverArtPath);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, []);

  if (!steamAppId && !customCoverArtPath?.trim()) {
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
