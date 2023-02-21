import { ReactNode } from 'react';

export default function DashboardCard({
  children,
  className = '',
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={`flex h-fit w-full flex-col justify-evenly gap-8 rounded-2xl bg-gray-850 p-4 ${className} border border-gray-faded/30`}
    >
      {children}
    </div>
  );
}
