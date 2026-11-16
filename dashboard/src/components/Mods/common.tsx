import { faCube, faDownload, faUser } from '@fortawesome/free-solid-svg-icons';
import RemoteIcon from 'components/Atoms/RemoteIcon';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstalledMod } from 'bindings/InstalledMod';
import { ModInstallResult } from 'bindings/ModInstallResult';
import { ModrinthSearchHit } from 'bindings/ModrinthSearchHit';
import { ModrinthVersionType } from 'bindings/ModrinthVersionType';
import Label, { LabelColor } from 'components/Atoms/Label';
import { formatCompact, useT } from 'i18n';
import { CountBadge, MonoText, Num, VersionTag } from 'components/Atoms/Values';
import { Fact, FactLine, NameList } from 'components/Values';
import Tooltip from 'rc-tooltip';
import { ReactNode, useState } from 'react';
import { safeImageUrl } from 'utils/markdown';
import { installedModName, installedModVersion } from 'utils/mods';
import { cn } from 'utils/util';
import { gameVersionsPhrase } from 'utils/values';

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

/** Shows `message` (text, or a list) in a tooltip when the children are hovered. */
export const WithTooltip = ({
  message,
  children,
}: {
  message: ReactNode;
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

/** The mods that need this one, for a tooltip: a heading with a count, one a line. */
export const NeededBy = ({ mods }: { mods: string[] }) => {
  const t = useT();
  return (
    <NameList
      groups={[
        {
          heading: t('mods.clientRequirement.neededBy'),
          items: mods.map((name) => ({ id: name, name })),
        },
      ]}
    />
  );
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
  const t = useT();
  switch (mod.client_requirement) {
    case 'required': {
      if (marks) return null;
      const label = (
        <Label size="small" color="blue" className="shrink-0 whitespace-nowrap">
          {t('mods.clientRequirement.required')}
        </Label>
      );
      return mod.required_by.length > 0 ? (
        <WithTooltip message={<NeededBy mods={mod.required_by} />}>
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

export /**
 * The Minecraft versions a file is for, as one plain phrase ("Minecraft
 * 1.21.1, 1.21, 1.20.6"; the game is never a version tag), then "+N" with the
 * rest in its title.
 */
const GameVersions = ({ versions }: { versions: string[] }) => {
  const shown = versions.slice(0, 3);
  const rest = versions.slice(3);
  return (
    <span className="inline-flex min-w-0 items-center gap-1">
      <span className="truncate">{gameVersionsPhrase(shown)}</span>
      {rest.length > 0 && (
        <CountBadge count={rest.length} plus title={rest.join('\n')} />
      )}
    </span>
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

/**
 * What an install did, as a list for its toast: the mods installed (name and
 * version), the files removed, the projects skipped with the core's reason,
 * and the mods players need too. Shown in a toast, so it reads the current
 * language when it renders.
 */
export const InstallResult = ({ result }: { result: ModInstallResult }) => {
  const t = useT();
  return (
    <NameList
      groups={[
        {
          heading: t('mods.installResult.group.installed'),
          tone: 'success',
          items: result.installed.map((mod) => {
            const version = installedModVersion(mod);
            return {
              id: mod.file_name,
              name: (
                <span className="flex min-w-0 items-center gap-1.5">
                  <span className="truncate">{installedModName(mod)}</span>
                  {version && <VersionTag version={version} max={18} />}
                </span>
              ),
            };
          }),
        },
        {
          heading: t('mods.installResult.group.removed'),
          items: result.removed.map((file) => ({
            id: file,
            name: <MonoText text={file} max={32} className="text-small" />,
          })),
        },
        {
          heading: t('mods.installResult.group.skipped'),
          // the reasons come from the core
          items: result.skipped.map((skip) => ({
            id: skip.project_id,
            name: (
              <MonoText
                text={skip.project_id}
                max={24}
                className="text-small"
              />
            ),
            reason: skip.reason,
          })),
        },
        {
          heading: t('mods.installResult.group.playersNeed'),
          items: result.installed.filter(playersNeed).map((mod) => ({
            id: mod.file_name,
            name: installedModName(mod),
          })),
        },
      ]}
    />
  );
};

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
        <FactLine>
          <Fact label={t('values.key.author')} icon={faUser}>
            {hit.author}
          </Fact>
          <Fact
            label={t('values.key.downloads')}
            icon={faDownload}
            title={t('mods.downloads', { count: hit.downloads })}
          >
            <Num value={formatCompact(hit.downloads)} />
          </Fact>
        </FactLine>
        <div className="line-clamp-2 text-medium text-white/50">
          {hit.description}
        </div>
      </div>
    </button>
  );
};
