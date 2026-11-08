import { ImgHTMLAttributes } from 'react';
import { cn } from 'utils/util';

/**
 * The Lodestone logo. The light theme gets a dark wordmark (logo_light.svg);
 * the switch is CSS only, so it is right from the first paint.
 */
export default function Logo({
  className,
  alt = 'Lodestone',
  ...props
}: ImgHTMLAttributes<HTMLImageElement>) {
  return (
    <>
      <img
        src="/logo.svg"
        alt={alt}
        className={cn(className, 'light:hidden')}
        {...props}
      />
      <img
        src="/logo_light.svg"
        alt={alt}
        className={cn(className, 'hidden light:inline')}
        {...props}
      />
    </>
  );
}
