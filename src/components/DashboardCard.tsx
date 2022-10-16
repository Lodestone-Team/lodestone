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
      className={`flex h-fit w-full flex-col justify-evenly gap-8 rounded-2xl bg-gray-900 px-10 pt-8 pb-10 ${className}`}
    >
      {children}
    </div>
  );
}
