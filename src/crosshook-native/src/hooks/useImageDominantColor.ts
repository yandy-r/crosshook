import { useEffect, useRef, useState } from 'react';

/**
 * Samples the dominant color from an image URL using an offscreen canvas.
 * Returns an `[r, g, b]` tuple or `null` while loading / on failure.
 */
export function useImageDominantColor(imageUrl: string | null): [number, number, number] | null {
  const [color, setColor] = useState<[number, number, number] | null>(null);
  const urlRef = useRef(imageUrl);

  useEffect(() => {
    urlRef.current = imageUrl;

    if (!imageUrl) {
      setColor(null);
      return;
    }

    const img = new Image();
    img.crossOrigin = 'anonymous';

    img.onload = () => {
      if (urlRef.current !== imageUrl) return;

      const canvas = document.createElement('canvas');
      // Down-sample to a small resolution for speed
      const size = 32;
      canvas.width = size;
      canvas.height = size;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      ctx.drawImage(img, 0, 0, size, size);
      const data = ctx.getImageData(0, 0, size, size).data;

      // Weighted average — favour the top third (header area) for a banner-appropriate tint
      let r = 0;
      let g = 0;
      let b = 0;
      let weight = 0;

      for (let i = 0; i < data.length; i += 4) {
        const pixelIndex = i / 4;
        const row = Math.floor(pixelIndex / size);
        // Top rows get more weight so the banner color matches the skyline / header of the art
        const w = row < size / 3 ? 3 : 1;
        r += data[i] * w;
        g += data[i + 1] * w;
        b += data[i + 2] * w;
        weight += w;
      }

      r = Math.round(r / weight);
      g = Math.round(g / weight);
      b = Math.round(b / weight);

      // Boost dark colors so they remain visible as UI accents on a dark background
      const luminance = 0.299 * r + 0.587 * g + 0.114 * b;
      if (luminance < 80) {
        const boost = 80 / Math.max(luminance, 1);
        r = Math.min(255, Math.round(r * boost));
        g = Math.min(255, Math.round(g * boost));
        b = Math.min(255, Math.round(b * boost));
      }

      setColor([r, g, b]);
    };

    img.onerror = () => {
      if (urlRef.current !== imageUrl) return;
      setColor(null);
    };

    img.src = imageUrl;
  }, [imageUrl]);

  return color;
}
