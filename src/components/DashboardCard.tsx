import { ReactNode } from 'react';

export default function DashboardCard({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-col items-start w-full gap-4 px-10 py-6 bg-gray-900 rounded-2xl justify-evenly h-fit">
      {children}
    </div>
  );
}
