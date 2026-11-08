import { faCube, faDownload } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstalledMod } from 'bindings/InstalledMod';
import { ModInstallResult } from 'bindings/ModInstallResult';
import { ModrinthSearchHit } from 'bindings/ModrinthSearchHit';
import { ModrinthVersionType } from 'bindings/ModrinthVersionType';
import Label, { LabelColor } from 'components/Atoms/Label';
import Tooltip from 'rc-tooltip';
import { ReactNode, useState } from 'react';
import { safeImageUrl } from 'utils/markdown';
import { formatCount, installedModName, installedModVersion } from 'utils/mods';
import { cn } from 'utils/util';

/** A project icon from Modrinth (https only), or a placeholder cube. */
export const ModIcon = ({
  url,
  className,
}: {
  url: string | null | undefined;
  className?: string;
}) => {
  const src = url ? safeImageUrl(url) : null;
  const [failed, setFailed] = useState(false);
  return (
    <div
      className={cn(
        'flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-md bg-gray-900 text-white/30',
        className
      )}
    >
      {src && !failed ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          className="size-full object-contain"
          onError={() => setFailed(true)}
        />
      ) : (
        <FontAwesomeIcon icon={faCube} />
      )}
    </div>
  );
};

/**
 * Shows `message` in a tooltip over the children when `show` is set, e.g.
 * why a button is disabled. The wrapper receives the mouse events that a
 * disabled button doesn't.
 */
export const DisabledTooltip = ({
  show,
  message,
  children,
}: {
  show: boolean;
  message: string;
  children: ReactNode;
}) => {
  if (!show) return <>{children}</>;
  return (
    <Tooltip
      overlay={<span>{message}</span>}
      placement="top"
      showArrow={false}
      trigger={['hover']}
      mouseEnterDelay={0}
    >
      <span className="inline-flex">{children}</span>
    </Tooltip>
  );
};

export const NO_WRITE_PERMISSION =
  "You don't have permission to change this instance's files";

const versionTypeColor: Record<ModrinthVersionType, LabelColor> = {
  release: 'green',
  beta: 'yellow',
  alpha: 'red',
};

export const VersionTypeLabel = ({ type }: { type: ModrinthVersionType }) => (
  <Label size="small" color={versionTypeColor[type] ?? 'gray'}>
    {type}
  </Label>
);

/** Opens a file picker and resolves with the chosen files (none if closed). */
export const pickFiles = (accept: string, multiple: boolean) =>
  new Promise<File[]>((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.multiple = multiple;
    input.onchange = () => resolve(Array.from(input.files ?? []));
    input.oncancel = () => resolve([]);
    input.click();
  });

const modLabel = (mod: InstalledMod) => {
  const version = installedModVersion(mod);
  return version
    ? `${installedModName(mod)} ${version}`
    : installedModName(mod);
};

/** The body of the toast after an install: installed, removed, skipped. */
export const InstallResultMessage = ({
  title,
  result,
}: {
  title: string;
  result: ModInstallResult;
}) => (
  <div className="flex flex-col gap-1">
    <div className="font-bold">{title}</div>
    {result.installed.length > 0 && (
      <div>Installed: {result.installed.map(modLabel).join(', ')}</div>
    )}
    {result.removed.length > 0 && (
      <div>Removed: {result.removed.join(', ')}</div>
    )}
    {result.skipped.length > 0 && (
      <div>
        Skipped:{' '}
        {result.skipped
          .map((skip) => `${skip.project_id} (${skip.reason})`)
          .join(', ')}
      </div>
    )}
  </div>
);

/** A Modrinth search result. */
export const ModrinthHitCard = ({
  hit,
  installed = false,
  onClick,
}: {
  hit: ModrinthSearchHit;
  installed?: boolean;
  onClick: () => void;
}) => (
  <button
    type="button"
    onClick={onClick}
    className={cn(
      'flex w-full flex-row items-start gap-3 rounded-lg border border-gray-faded/30 bg-gray-800 p-3 text-left',
      'hover:border-gray-faded/50 hover:bg-gray-700',
      'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden'
    )}
  >
    <ModIcon url={hit.icon_url} className="size-12" />
    <div className="flex min-w-0 grow flex-col gap-0.5">
      <div className="flex min-w-0 flex-row items-center gap-2">
        <span className="truncate text-h3 font-bold text-gray-300">
          {hit.title}
        </span>
        <span className="shrink-0 truncate text-small text-white/50">
          by {hit.author}
        </span>
        {installed && (
          <Label size="small" color="green" className="shrink-0">
            Installed
          </Label>
        )}
      </div>
      <div className="line-clamp-2 text-medium text-white/50">
        {hit.description}
      </div>
    </div>
    <div className="flex shrink-0 flex-row items-center gap-1 text-small text-white/50">
      <FontAwesomeIcon icon={faDownload} />
      {formatCount(hit.downloads)}
    </div>
  </button>
);
