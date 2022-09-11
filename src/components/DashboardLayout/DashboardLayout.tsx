// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import { useRouter } from 'next/router';
import { useLayoutEffect } from 'react';
import Split from 'react-split';
import { useWindowSize } from 'usehooks-ts';
import { LodestoneContext } from 'data/LodestoneContext';
import axios from 'axios';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const { address, port } = router.query;
  const { width: windowWidth } = useWindowSize();
  const minWidth = (windowWidth / 12) * 1.5;
  const maxWidth = (windowWidth / 12) * 4;

  const protocol = 'http';
  const apiVersion = 'v1';

  // set axios defaults
  useLayoutEffect(() => {
    if (!router.isReady) return;
    axios.defaults.baseURL = `${protocol}://${address}:${port ?? 3000}/api/${apiVersion}`;
  }, [address, port, router.isReady]);

  return (
    <LodestoneContext.Provider
      value={{
        address: address as string,
        port: port as string,
        protocol,
        apiVersion,
        isReady: router.isReady,
      }}
    >
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
    </LodestoneContext.Provider>
  );
}
