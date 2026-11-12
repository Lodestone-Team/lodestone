import { faCube, faDownload } from '@fortawesome/free-solid-svg-icons';
import RemoteIcon from 'components/Atoms/RemoteIcon';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstalledMod } from 'bindings/InstalledMod';
import { ModInstallResult } from 'bindings/ModInstallResult';
import { ModrinthSearchHit } from 'bindings/ModrinthSearchHit';
import { ModrinthVersionType } from 'bindings/ModrinthVersionType';
import Label, { LabelColor } from 'components/Atoms/Label';
import { t as translate, useI18n, useT } from 'i18n';
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
        <RemoteIcon
          src={src}
          alt=""
          loading="lazy"
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
  return <WithTooltip message={message}>{children}</WithTooltip>;
};

/** Shows `message` in a tooltip when the children are hovered. */
export const WithTooltip = ({
  message,
  children,
}: {
  message: string;
  children: ReactNode;
}) => (
  <Tooltip
    // long hints wrap instead of spanning the page
    overlay={
      <span className="block max-w-xs whitespace-normal">{message}</span>
    }
    placement="top"
    showArrow={false}
    trigger={['hover']}
    mouseEnterDelay={0}
  >
    <span className="inline-flex">{children}</span>
  </Tooltip>
);

/**
 * Whether players need a Modrinth project too, from its `client_side`:
 * "Players need this too" when required, a quiet "Server only" when the
 * client doesn't support it, and nothing otherwise.
 */
export const ClientSideLabel = ({ clientSide }: { clientSide: string }) => {
  const t = useT();
  if (clientSide === 'required')
    return (
      <Label size="small" color="blue" className="shrink-0 whitespace-nowrap">
        {t('mods.clientSide.playersNeedToo')}
      </Label>
    );
  if (clientSide === 'unsupported')
    return (
      <span className="shrink-0 text-small whitespace-nowrap text-white/30">
        {t('mods.clientSide.serverOnly')}
      </span>
    );
  return null;
};

/**
 * Whether players need an installed mod to join (CLIENT_MODS_SPEC 2.1).
 * Optional mods show nothing; unknown ones a quiet note.
 */
export const ClientRequirementLabel = ({
  mod,
  marks = false,
}: {
  mod: InstalledMod;
  /** the row has a "For players" box, which shows "required" itself */
  marks?: boolean;
}) => {
  const { t, formatList } = useI18n();
  switch (mod.client_requirement) {
    case 'required': {
      if (marks) return null;
      const label = (
        <Label size="small" color="blue" className="shrink-0 whitespace-nowrap">
          {t('mods.clientRequirement.required')}
        </Label>
      );
      return mod.required_by.length > 0 ? (
        <WithTooltip
          message={t('mods.clientRequirement.neededBy', {
            mods: formatList(mod.required_by),
          })}
        >
          {label}
        </WithTooltip>
      ) : (
        label
      );
    }
    case 'unsupported':
      return (
        <span className="shrink-0 text-small whitespace-nowrap text-white/30">
          {t('mods.clientSide.serverOnly')}
        </span>
      );
    case 'unknown':
      return (
        <WithTooltip
          message={
            marks
              ? mod.modrinth
                ? t('mods.clientRequirement.unknownModrinthMark')
                : t('mods.clientRequirement.unknownNotOnModrinthMark')
              : mod.modrinth
                ? t('mods.clientRequirement.unknownModrinth')
                : t('mods.clientRequirement.unknownNotOnModrinth')
          }
        >
          <span className="shrink-0 cursor-default text-small whitespace-nowrap text-white/30">
            {t('common.unknown')}
          </span>
        </WithTooltip>
      );
    default:
      return null;
  }
};

/** Whether players need this freshly installed mod. Plugins never. */
const playersNeed = (mod: InstalledMod) =>
  mod.client_requirement === 'required' ||
  (mod.client_requirement !== 'unsupported' &&
    mod.modrinth?.client_side === 'required');

const versionTypeColor: Record<ModrinthVersionType, LabelColor> = {
  release: 'green',
  beta: 'yellow',
  alpha: 'red',
};

const versionTypeLabel: Record<
  ModrinthVersionType,
  `mods.versionType.${ModrinthVersionType}`
> = {
  release: 'mods.versionType.release',
  beta: 'mods.versionType.beta',
  alpha: 'mods.versionType.alpha',
};

export const VersionTypeLabel = ({ type }: { type: ModrinthVersionType }) => {
  const t = useT();
  // a type Modrinth adds later is shown as it is
  const labelKey = versionTypeLabel[type] as
    `mods.versionType.${ModrinthVersionType}` | undefined;
  return (
    <Label
      size="small"
      color={versionTypeColor[type] ?? 'gray'}
      className="whitespace-nowrap"
    >
      {labelKey ? t(labelKey) : type}
    </Label>
  );
};

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

/**
 * The body of the toast after an install: installed, removed, skipped.
 * Shown in a toast, so it reads the current language when it renders.
 */
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
      <div>
        {translate('mods.installResult.installed', {
          mods: result.installed.map(modLabel).join(', '),
        })}
      </div>
    )}
    {result.removed.length > 0 && (
      <div>
        {translate('mods.installResult.removed', {
          files: result.removed.join(', '),
        })}
      </div>
    )}
    {result.skipped.length > 0 && (
      <div>
        {/* the reasons come from the core */}
        {translate('mods.installResult.skipped', {
          projects: result.skipped
            .map((skip) => `${skip.project_id} (${skip.reason})`)
            .join(', '),
        })}
      </div>
    )}
    {result.installed.some(playersNeed) && (
      <div>
        {translate('mods.installResult.playersNeed', {
          mods: result.installed
            .filter(playersNeed)
            .map(installedModName)
            .join(', '),
        })}
      </div>
    )}
  </div>
);

/** A Modrinth search result. */
export const ModrinthHitCard = ({
  hit,
  installed = false,
  showClientSide = false,
  onClick,
}: {
  hit: ModrinthSearchHit;
  installed?: boolean;
  /** for mods: show whether players need the project too */
  showClientSide?: boolean;
  onClick: () => void;
}) => {
  const t = useT();
  return (
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
            {t('mods.byAuthor', { author: hit.author })}
          </span>
          {installed && (
            <Label
              size="small"
              color="green"
              className="shrink-0 whitespace-nowrap"
            >
              {t('mods.installedLabel')}
            </Label>
          )}
          {showClientSide && <ClientSideLabel clientSide={hit.client_side} />}
        </div>
        <div className="line-clamp-2 text-medium text-white/50">
          {hit.description}
        </div>
      </div>
      <div
        className="flex shrink-0 flex-row items-center gap-1 text-small text-white/50"
        title={t('mods.downloads', { count: hit.downloads })}
      >
        <FontAwesomeIcon icon={faDownload} />
        {formatCount(hit.downloads)}
      </div>
    </button>
  );
};
