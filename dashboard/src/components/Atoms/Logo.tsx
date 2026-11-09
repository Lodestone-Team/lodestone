import { ImgHTMLAttributes } from 'react';
import { cn } from 'utils/util';
import { asset } from 'utils/basePath';

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
        src={asset('/logo.svg')}
        alt={alt}
        className={cn(className, 'light:hidden')}
        {...props}
      />
      <img
        src={asset('/logo_light.svg')}
        alt={alt}
        className={cn(className, 'hidden light:inline')}
        {...props}
      />
    </>
  );
}
