import { ReactNode } from 'react';
import { cn } from 'utils/util';

export default function DashboardCard({
  children,
  className = '',
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(`flex h-fit w-full flex-col justify-evenly gap-8 rounded-2xl border border-gray-faded/30 bg-gray-850 p-4`, className)}
    >
      {children}
    </div>
  );
}
