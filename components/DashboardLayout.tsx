// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex flex-row w-screen h-screen text-bright font-body">
      <LeftNav />
      <div className="w-10/12 h-full">
        <TopNav />
        {children}
      </div>
    </div>
  )
}
