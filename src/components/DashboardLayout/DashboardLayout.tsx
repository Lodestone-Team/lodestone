// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import { useRouter } from 'next/router';
import { setAddress, setLoading, setPort } from 'data/ClientInfo';
import { useEffect } from 'react';
import { useAppDispatch } from 'utils/hooks';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const { address, port, uuid } = router.query;
  const dispatch = useAppDispatch();

  useEffect(() => {
    if(address)
      dispatch(setAddress(address as string));

    if(port){
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

      dispatch(setPort(parseInt(port as string)));
    }
    
    dispatch(setLoading(!router.isReady));
  }, [address, port, dispatch]);

  return (
    <div className="flex flex-row w-screen h-screen text-gray-300 font-body">
      <LeftNav />
      <div className="w-10/12 h-full">
        <TopNav />
        {children}
      </div>
    </div>
  );
}
