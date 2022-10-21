// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import Split from 'react-split';
import { useWindowSize } from 'usehooks-ts';
import { useEventStream } from 'data/EventStream';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  useEventStream();

  const { width: windowWidth } = useWindowSize();
  const minWidth = (windowWidth / 12) * 1.5;
  const maxWidth = (windowWidth / 12) * 4;

  return (
    <div className="flex flex-col h-screen">
      <TopNav />
      <Split
        sizes={[10, 90]}
        minSize={[minWidth, 0]}
        maxSize={[maxWidth, Infinity]}
        snapOffset={0}
        gutterSize={0}
        className="flex flex-row items-stretch w-screen min-h-0 text-gray-300 bg-gray-800 grow"
      >
        <LeftNav />
        <div>{children}</div>
      </Split>
    </div>
  );
}
