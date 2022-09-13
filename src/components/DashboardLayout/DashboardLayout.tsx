// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import Split from 'react-split';
import { useWindowSize } from 'usehooks-ts';
import { useReactQuerySubscription } from 'data/LodestoneStream';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  useReactQuerySubscription();
  
  const { width: windowWidth } = useWindowSize();
  const minWidth = (windowWidth / 12) * 1.5;
  const maxWidth = (windowWidth / 12) * 4;
  

  return (
    <Split
      sizes={[16, 84]}
      minSize={[minWidth, 0]}
      maxSize={[maxWidth, Infinity]}
      snapOffset={0}
      gutterSize={0}
      className="flex flex-row w-full min-h-screen text-gray-300 bg-gray-800"
    >
      <LeftNav />
      <div className="flex flex-col">
        <TopNav />
        {children}
      </div>
    </Split>
  );
}
