import { useContext } from 'react';
import Tooltip from 'rc-tooltip';
import clsx from 'clsx';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faPlug,
  faPlugCircleCheck,
  faPlugCircleExclamation,
  faPlugCircleXmark,
} from '@fortawesome/free-solid-svg-icons';
import type { IconDefinition } from '@fortawesome/fontawesome-svg-core';
import {
  CoreConnectionStatus as Status,
  LodestoneContext,
} from 'data/LodestoneContext';
import { useCoreInfo } from 'data/SystemInfo';

const statusDisplay: Record<
  Status,
  { label: string; detail: string; icon: IconDefinition; color: string }
> = {
  success: {
    label: 'Connected',
    detail: 'Receiving live updates',
    icon: faPlugCircleCheck,
    color: 'text-green-300',
  },
  loading: {
    label: 'Connecting…',
    detail: 'Waiting for the core to respond',
    icon: faPlug,
    color: 'text-gray-400',
  },
  degraded: {
    label: 'Degraded',
    detail: 'Live updates unavailable, refresh to update',
    icon: faPlugCircleExclamation,
    color: 'text-yellow-300',
  },
  error: {
    label: 'Disconnected',
    detail: 'Cannot reach the core',
    icon: faPlugCircleXmark,
    color: 'text-red-200',
  },
};

const osNames: Record<string, string> = {
  macos: 'macOS',
  windows: 'Windows',
  linux: 'Linux',
};
const archNames: Record<string, string> = { aarch64: 'arm64' };

const formatUptime = (upSince: number) => {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - upSince));
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
};

/**
 * The connected core's name with a connection status icon. Hovering the row shows
 * details about the connection and the core.
 */
export default function CoreConnectionStatus({
  className,
}: {
  className?: string;
}) {
  const { core, coreConnectionStatus } = useContext(LodestoneContext);
  const { data: coreInfo } = useCoreInfo();
  const status = statusDisplay[coreConnectionStatus];

  const rows: [string, string][] = [
    ['Address', `${core.protocol}://${core.address}:${core.port}`],
  ];
  if (coreInfo) {
    rows.push(
      ['Version', coreInfo.version],
      ['Host', coreInfo.host_name],
      [
        'System',
        `${osNames[coreInfo.os] ?? coreInfo.os} ${archNames[coreInfo.arch] ?? coreInfo.arch}`,
      ],
      ['Uptime', formatUptime(coreInfo.up_since)]
    );
  }

  const details = (
    <div className="flex flex-col gap-1 py-1">
      <div className="flex flex-row items-center gap-1.5">
        <FontAwesomeIcon icon={status.icon} className={status.color} />
        <span className={clsx('font-bold', status.color)}>{status.label}</span>
      </div>
      <span className="text-gray-400">{status.detail}</span>
      <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
        {rows.map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-gray-400">{label}</dt>
            <dd className="text-gray-300">{value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );

  return (
    <Tooltip
      overlay={details}
      placement="topLeft"
      showArrow={false}
      trigger={['hover', 'focus']}
      mouseEnterDelay={0.2}
    >
      <div
        tabIndex={0}
        aria-label={`${coreInfo?.core_name ?? 'Core'}: ${status.label}`}
        className={clsx(
          'flex w-full cursor-default flex-row items-center gap-2 rounded-md px-2 py-1.5',
          'text-small font-medium text-white/50 hover:bg-gray-800',
          'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden',
          className
        )}
      >
        <FontAwesomeIcon
          icon={status.icon}
          className={clsx('shrink-0', status.color)}
        />
        <span className="truncate">{coreInfo?.core_name ?? '...'}</span>
      </div>
    </Tooltip>
  );
}
