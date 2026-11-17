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
import { useT, type MessageKey, type TFunction } from 'i18n';
import {
  MonoText,
  Num,
  StatusBadge,
  type StatusTone,
  VersionTag,
} from 'components/Atoms/Values';
import { FactList, type FactRow } from 'components/Values';

const statusDisplay = {
  success: {
    labelKey: 'shell.connection.connected',
    detailKey: 'shell.connection.connectedDetail',
    icon: faPlugCircleCheck,
    color: 'text-green-300',
    tone: 'success',
  },
  loading: {
    labelKey: 'shell.connection.connecting',
    detailKey: 'shell.connection.connectingDetail',
    icon: faPlug,
    color: 'text-gray-400',
    tone: 'neutral',
  },
  degraded: {
    labelKey: 'shell.connection.degraded',
    detailKey: 'shell.connection.degradedDetail',
    icon: faPlugCircleExclamation,
    color: 'text-yellow-300',
    tone: 'warning',
  },
  error: {
    labelKey: 'shell.connection.disconnected',
    detailKey: 'shell.connection.disconnectedDetail',
    icon: faPlugCircleXmark,
    color: 'text-red-200',
    tone: 'danger',
  },
} as const satisfies Record<
  Status,
  {
    labelKey: MessageKey;
    detailKey: MessageKey;
    icon: IconDefinition;
    color: string;
    tone: StatusTone;
  }
>;

const osNames: Record<string, string> = {
  macos: 'macOS',
  windows: 'Windows',
  linux: 'Linux',
};
const archNames: Record<string, string> = { aarch64: 'arm64' };

const formatUptime = (t: TFunction, upSince: number) => {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - upSince));
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return t('shell.connection.uptimeDays', { days, hours });
  if (hours > 0) return t('shell.connection.uptimeHours', { hours, minutes });
  return t('shell.connection.uptimeMinutes', { minutes });
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
  const t = useT();
  const { core, coreConnectionStatus } = useContext(LodestoneContext);
  const { data: coreInfo } = useCoreInfo();
  const status = statusDisplay[coreConnectionStatus];
  const statusLabel = t(status.labelKey);

  const rows: FactRow[] = [
    {
      label: t('shell.connection.address'),
      value: (
        <MonoText
          text={`${core.protocol}://${core.address}:${core.port}`}
          max={40}
          className="text-small"
        />
      ),
    },
  ];
  if (coreInfo) {
    rows.push(
      {
        label: t('shell.connection.version'),
        value: <VersionTag version={coreInfo.version} />,
      },
      { label: t('shell.connection.host'), value: coreInfo.host_name },
      {
        label: t('shell.connection.system'),
        value: `${osNames[coreInfo.os] ?? coreInfo.os} ${archNames[coreInfo.arch] ?? coreInfo.arch}`,
      },
      {
        label: t('shell.connection.uptime'),
        value: <Num value={formatUptime(t, coreInfo.up_since)} />,
      }
    );
  }

  const details = (
    <div className="flex flex-col gap-1 py-1">
      <div className="flex flex-row items-center gap-1.5">
        <FontAwesomeIcon
          icon={status.icon}
          className={status.color}
          aria-hidden="true"
        />
        <StatusBadge tone={status.tone}>{statusLabel}</StatusBadge>
      </div>
      <span className="text-gray-400">{t(status.detailKey)}</span>
      <FactList compact className="mt-1 text-small" rows={rows} />
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
        aria-label={t('shell.connection.ariaLabel', {
          core: coreInfo?.core_name ?? t('shell.connection.coreFallback'),
          status: statusLabel,
        })}
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
