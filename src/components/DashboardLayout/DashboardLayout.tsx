// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import { useRouter } from 'next/router';
import { setapiUrl, setLoading } from 'data/ClientInfo';
import { useEffect } from 'react';
import { useAppDispatch } from 'utils/hooks';
import Split from 'react-split';
import { useWindowSize } from 'usehooks-ts';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const { address, port } = router.query;
  const dispatch = useAppDispatch();
  const { width: windowWidth } = useWindowSize();
  const minWidth = (windowWidth / 12) * 1.5;
  const maxWidth = (windowWidth / 12) * 4;

  useEffect(() => {
    if (!router.isReady) return;

    // try to parse port as number
    let portNumber = 3000;

    try {
      portNumber = parseInt(port as string);
      if (portNumber < 1 || portNumber > 65535 || isNaN(portNumber)) {
        portNumber = 3000;
        // TODO: redirect to error page
      }
    } catch (e) {
      console.log(`Invalid port number: ${port}`);
    }

    dispatch(setapiUrl(`http://${address ? address : 'localhost'}:${portNumber}`));

    dispatch(setLoading(!router.isReady));
  }, [address, port, dispatch, router.isReady]);

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
