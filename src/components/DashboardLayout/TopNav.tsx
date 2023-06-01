import { LodestoneContext } from 'data/LodestoneContext';
import { Fragment, useContext, useEffect, useState } from 'react';
import {
  faCaretDown,
  faArrowRightArrowLeft,
  faBell,
  faCog,
  faRightFromBracket,
  faUser,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstanceContext } from 'data/InstanceContext';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { CoreInfo, useCoreInfo } from 'data/SystemInfo';
import { AxiosError } from 'axios';
import Label, { LabelColor } from 'components/Atoms/Label';

import TopBanner from 'components/Atoms/TopBanner';
import { NotificationPopover } from './NotificationPopover';

export default function TopNav() {
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);

  const { token, setToken, core, coreConnectionStatus } =
    useContext(LodestoneContext);

  const { data: coreData } = useCoreInfo();

  const statusMap = {
    loading: 'Connecting',
    error: 'Error',
    success: 'Connected',
    degraded: 'Degraded',
  };

  const colorMap: Record<string, LabelColor> = {
    loading: 'gray',
    error: 'red',
    success: 'green',
    degraded: 'yellow',
  };

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
      <div className="flex w-full shrink-0 select-none flex-row items-center justify-end gap-4 border-b border-gray-faded/30 bg-gray-800 px-4 py-2">
        <div className="grow">
          <img
            src="/logo.svg"
            alt="logo"
            className="w-32 hover:cursor-pointer"
            onClick={() => {
              setSearchParam('instance', undefined);
              setSearchParam('user', undefined);
              setPathname('/');
            }}
          />
        </div>
        <div className="flex flex-row items-baseline gap-1">
          <p className="text-center text-medium font-medium text-white/50">
            {coreData?.core_name ?? '...'}:
          </p>
          <Label
            size="small"
            color={colorMap[coreConnectionStatus]}
            className="w-20 text-center"
          >
            {statusMap[coreConnectionStatus]}
          </Label>
        </div>
        <NotificationPopover />
      </div>
    </>
  );
}
