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
  const { address, port, uuid } = router.query;
  const dispatch = useAppDispatch();
  const { width: windowWidth, height: windowHeight } = useWindowSize();
  const minWidth = (windowWidth / 12) * 1.5;
  const maxWidth = (windowWidth / 12) * 4;

  useEffect(() => {
    if (!address || !port) return;

    // try to parse port as number
    let portNumber = 3000;

    try {
      portNumber = parseInt(port as string);
      // quick error check for valid port number
      if (portNumber < 1 || portNumber > 65535) {
        portNumber = 3000;
        // TODO: redirect to error page
      }
    } catch (e) {
      console.log(`Invalid port number: ${port}`);
    }

    dispatch(setapiUrl(`http://${address}:${portNumber}`));

    dispatch(setLoading(!router.isReady));
  }, [address, port, dispatch]);

  return (
    <Split
      sizes={[16, 84]}
      minSize={[minWidth, 0]}
      maxSize={[maxWidth, Infinity]}
      snapOffset={0}
      className="flex flex-row w-screen h-screen text-gray-300 font-body"
    >
      <LeftNav />
      <div className="h-full">
        <TopNav />
        {children}
      </div>
    </Split>
  );
}
