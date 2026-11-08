import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import { BrowserLocationContext } from 'data/BrowserLocationContext';

import TopBanner from 'components/Atoms/TopBanner';
import Logo from 'components/Atoms/Logo';
import clsx from 'clsx';
import { isTauriMacOS, useIsDevBuild } from 'utils/tauriUtil';
import { NotificationPopover } from './NotificationPopover';

export default function TopNav() {
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const { coreConnectionStatus } = useContext(LodestoneContext);

  // In the macOS app this bar replaces the native title bar: its empty areas move the
  // window, and the traffic-light buttons sit in the left column.
  const dragRegion = isTauriMacOS || undefined;
  const isDevBuild = useIsDevBuild();

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
        className={clsx(
          'grid w-full shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b bg-gray-800 px-4 light:bg-gray-850 py-2 select-none',
          // An amber edge marks local development builds, matching the DEV badge and app icon.
          isDevBuild ? 'border-yellow-300/70' : 'border-gray-faded/30'
        )}
      >
        <div className="h-full" data-tauri-drag-region={dragRegion} />
        <div className="relative">
          <Logo
            alt="Lodestone"
            className="w-28 hover:cursor-pointer"
            onClick={() => {
              setSearchParam('instance', undefined);
              setSearchParam('user', undefined);
              setPathname('/');
            }}
          />
          {isDevBuild && (
            // Positioned outside the flow so the logo stays centred.
            <span
              title="Local development build"
              className="absolute top-1/2 left-full ml-2 -translate-y-1/2 rounded-sm bg-yellow-300 px-1.5 light:bg-pill-yellow light:ring-1 light:ring-yellow-300/40 py-px text-small font-bold tracking-wide text-on-accent-dark"
            >
              DEV
            </span>
          )}
        </div>
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
