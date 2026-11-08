import { useEffect, useRef, useState } from 'react';
import { cn } from 'utils/util';

/** Icons smaller than this (in source pixels) are pixel art: scale them up crisply. */
const PIXEL_ART_MAX = 64;

const isGif = (src: string) => {
  try {
    return new URL(src).pathname.toLowerCase().endsWith('.gif');
  } catch {
    return false;
  }
};

/**
 * A modpack or mod icon from Modrinth's CDN.
 *
 * - Animated GIFs are shown still, on their first frame. A looping icon in a list
 *   distracts, and some (signal bars, spinners) look like a loading indicator.
 * - Small icons, which are usually pixel art, are scaled up with sharp pixels
 *   instead of being blurred.
 *
 * `src` must already be filtered (https, allowed host); this component doesn't
 * check it.
 */
export default function RemoteIcon({
  src,
  alt,
  className,
  loading,
  onError,
}: {
  src: string;
  alt: string;
  className?: string;
  loading?: 'lazy' | 'eager';
  onError?: () => void;
}) {
  const [small, setSmall] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const gif = isGif(src);

  // Draw the GIF's first frame onto a canvas. The canvas is never read back, so
  // a cross-origin image is fine.
  useEffect(() => {
    if (!gif) return;
    const image = new Image();
    image.referrerPolicy = 'no-referrer';
    image.onload = () => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      canvas.getContext('2d')?.drawImage(image, 0, 0);
      setSmall(image.naturalWidth < PIXEL_ART_MAX);
    };
    image.onerror = () => onError?.();
    image.src = src;
    return () => {
      image.onload = null;
      image.onerror = null;
    };
    // onError is a callback from the parent; reloading when it changes is not wanted
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [src, gif]);

  const pixelated = small && '[image-rendering:pixelated]';

  if (gif) {
    return (
      <canvas
        ref={canvasRef}
        role="img"
        aria-label={alt}
        className={cn(className, pixelated)}
      />
    );
  }
  return (
    <img
      src={src}
      alt={alt}
      loading={loading}
      referrerPolicy="no-referrer"
      className={cn(className, pixelated)}
      onLoad={(e) => setSmall(e.currentTarget.naturalWidth < PIXEL_ART_MAX)}
      onError={onError}
    />
  );
}
