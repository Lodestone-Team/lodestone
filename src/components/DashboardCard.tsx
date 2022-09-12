import { ReactNode } from 'react';

export default function DashboardCard({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-col items-start w-full px-10 py-8 bg-gray-900 rounded-2xl justify-evenly h-fit">
      {children}
    </div>
  );
}
