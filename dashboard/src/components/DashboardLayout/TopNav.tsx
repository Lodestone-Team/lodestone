import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import { BrowserLocationContext } from 'data/BrowserLocationContext';

import TopBanner from 'components/Atoms/TopBanner';
import { isTauriMacOS } from 'utils/tauriUtil';
import { NotificationPopover } from './NotificationPopover';

export default function TopNav() {
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const { coreConnectionStatus } = useContext(LodestoneContext);

  // In the macOS app this bar replaces the native title bar: its empty areas move the
  // window, and the traffic-light buttons sit in the left column.
  const dragRegion = isTauriMacOS || undefined;

  return (
    <>
      {coreConnectionStatus === 'degraded' && (
        <TopBanner intention="warning">
          <p>
            <a
              href="#"
              className="font-bold hover:underline"
              onClick={() => {
                window.location.reload();
              }}
            >
              Refresh
            </a>{' '}
            to get the latest data on Firefox.{' '}
            <a
              href="https://github.com/Lodestone-Team/lodestone/wiki/Known-Issues#firefox"
              target="_blank"
              rel="noreferrer"
              className="hover:underline"
            >
              Learn more
            </a>
          </p>
        </TopBanner>
      )}
      <div
        data-tauri-drag-region={dragRegion}
        // Equal side columns keep the logo centred in the window.
        className="grid w-full shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-gray-faded/30 bg-gray-800 px-4 py-2 select-none"
      >
        <div className="h-full" data-tauri-drag-region={dragRegion} />
        <img
          src="/logo.svg"
          alt="Lodestone"
          className="w-28 hover:cursor-pointer"
          onClick={() => {
            setSearchParam('instance', undefined);
            setSearchParam('user', undefined);
            setPathname('/');
          }}
        />
        <div
          className="flex h-full flex-row items-center justify-end"
          data-tauri-drag-region={dragRegion}
        >
          <NotificationPopover />
        </div>
      </div>
    </>
  );
}
