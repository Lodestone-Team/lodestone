// A persistent banner at the top of the page displaying a message.

import { cn } from 'utils/util';

const TopBanner = ({
  children,
  className,
  intention = 'info',
}: {
  children: React.ReactNode;
  className?: string;
  intention?: 'info' | 'warning' | 'error';
}) => {
  return (
    <div
      className={cn(
        ' flex h-8 w-full items-center justify-center border-b border-gray-faded/30 text-medium font-medium text-gray-900',
        {
          'bg-gray-800': intention === 'info',
          'bg-yellow-300': intention === 'warning',
          'bg-red-300': intention === 'error',
        },
        className
      )}
    >
      {children}
    </div>
  );
};

export default TopBanner;
