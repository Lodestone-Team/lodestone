// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from 'components/LeftNav';
import TopNav from 'components/TopNav';

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex flex-row w-screen h-screen text-gray-300 font-body">
      <LeftNav />
      <div className="w-10/12 h-full">
        <TopNav />
        {children}
      </div>
    </div>
  )
}
