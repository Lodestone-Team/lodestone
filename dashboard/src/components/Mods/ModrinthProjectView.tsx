import {
  faArrowLeft,
  faArrowUpRightFromSquare,
  faDownload,
  faHeart,
  faServer,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { InstalledModList } from 'bindings/InstalledModList';
import { ModKind } from 'bindings/ModKind';
import { ModLoader } from 'bindings/ModLoader';
import { ModrinthProject } from 'bindings/ModrinthProject';
import { ModrinthVersion } from 'bindings/ModrinthVersion';
import Button from 'components/Atoms/Button';
import Checkbox from 'components/Atoms/Checkbox';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import Label from 'components/Atoms/Label';
import WarningAlert from 'components/Atoms/WarningAlert';
import SafeMarkdown from 'components/SafeMarkdown';
import {
  installMod,
  invalidateInstalledMods,
  ModrinthVersionFilter,
  useModrinthProject,
  useModrinthVersion,
  useModrinthVersions,
} from 'data/Mods';
import { useMemo, useState } from 'react';
import { toast } from 'react-toastify';
import { safeImageUrl, safeLinkUrl } from 'utils/markdown';
import {
  compatibleLoaders,
  formatCount,
  formatDate,
  isVersionCompatible,
  modNoun,
  modrinthProjectUrl,
} from 'utils/mods';
import { errorToString } from 'utils/util';
import {
  DisabledTooltip,
  InstallResultMessage,
  ModIcon,
  NO_WRITE_PERMISSION,
  VersionTypeLabel,
} from './common';

/** The instance a project view installs into. */
export type InstallTarget = {
  uuid: string;
  kind: ModKind;
  loader: ModLoader;
  gameVersion: string;
  installed: InstalledModList | undefined;
  canWrite: boolean;
};

const PAGE_SIZE = 20;

const ProjectLinks = ({ project }: { project: ModrinthProject }) => {
  const links: Array<[string, string | null]> = [
    [
      'Modrinth',
      safeLinkUrl(modrinthProjectUrl(project.project_type, project.slug)),
    ],
    ['Source', project.source_url ? safeLinkUrl(project.source_url) : null],
    ['Issues', project.issues_url ? safeLinkUrl(project.issues_url) : null],
    ['Wiki', project.wiki_url ? safeLinkUrl(project.wiki_url) : null],
    ['Discord', project.discord_url ? safeLinkUrl(project.discord_url) : null],
  ];
  return (
    <div className="flex flex-row flex-wrap gap-3 text-medium">
      {links.map(
        ([label, href]) =>
          href && (
            <a
              key={label}
              href={href}
              target="_blank"
              rel="noreferrer noopener"
              className="flex flex-row items-center gap-1 text-blue-200 hover:underline"
            >
              {label}
              <FontAwesomeIcon
                icon={faArrowUpRightFromSquare}
                className="text-small"
              />
            </a>
          )
      )}
    </div>
  );
};

const Gallery = ({ project }: { project: ModrinthProject }) => {
  const images = project.gallery
    .map((image) => ({
      ...image,
      src: safeImageUrl(image.url),
      href: safeLinkUrl(image.url),
    }))
    .filter((image) => image.src && image.href);
  if (images.length === 0) return null;
  return (
    <div className="flex flex-row gap-2 overflow-x-auto pb-1">
      {images.map((image, i) => (
        <a
          key={i}
          href={image.href ?? undefined}
          target="_blank"
          rel="noreferrer noopener"
          title={image.title ?? undefined}
          className="shrink-0 overflow-hidden rounded-md border border-gray-faded/30 hover:border-gray-faded/50"
        >
          <img
            src={image.src ?? undefined}
            alt={image.title ?? ''}
            loading="lazy"
            referrerPolicy="no-referrer"
            className="h-24 w-auto max-w-xs object-cover"
          />
        </a>
      ))}
    </div>
  );
};

const GameVersions = ({ versions }: { versions: string[] }) => {
  const shown = versions.slice(0, 3);
  const rest = versions.length - shown.length;
  return (
    <span title={versions.join(', ')}>
      {shown.join(', ')}
      {rest > 0 && ` +${rest} more`}
    </span>
  );
};

const VersionRow = ({
  version,
  target,
  installedHere,
  installing,
  onInstall,
}: {
  version: ModrinthVersion;
  target: InstallTarget;
  installedHere: boolean;
  installing: boolean;
  onInstall: (version: ModrinthVersion) => void;
}) => {
  const compatible = isVersionCompatible(
    version,
    target.loader,
    target.gameVersion
  );
  return (
    <div className="flex flex-row items-center gap-3 bg-gray-800 px-4 py-2">
      <div className="flex min-w-0 grow flex-col">
        <div className="flex min-w-0 flex-row items-center gap-2">
          <span className="truncate text-medium font-bold text-gray-300">
            {version.version_number}
          </span>
          <VersionTypeLabel type={version.version_type} />
          {installedHere && (
            <Label size="small" color="green">
              Installed
            </Label>
          )}
          {!compatible && (
            <Label size="small" color="gray">
              Not for {target.loader} {target.gameVersion}
            </Label>
          )}
        </div>
        <div className="truncate text-small text-white/50">
          <GameVersions versions={version.game_versions} /> ·{' '}
          {version.loaders.join(', ')} · {formatDate(version.date_published)}
        </div>
      </div>
      <DisabledTooltip show={!target.canWrite} message={NO_WRITE_PERMISSION}>
        <Button
          label={installedHere ? 'Reinstall' : 'Install'}
          icon={faDownload}
          intention={compatible ? 'primary' : 'info'}
          size="slim"
          loading={installing}
          disabled={!target.canWrite}
          onClick={() => onInstall(version)}
        />
      </DisabledTooltip>
    </div>
  );
};

const ProjectVersions = ({
  project,
  linkedVersionId,
  target,
}: {
  project: ModrinthProject;
  linkedVersionId: string | null;
  target: InstallTarget;
}) => {
  const queryClient = useQueryClient();
  const [showAll, setShowAll] = useState(false);
  const [shown, setShown] = useState(PAGE_SIZE);
  const [withDependencies, setWithDependencies] = useState(true);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [confirmVersion, setConfirmVersion] = useState<ModrinthVersion | null>(
    null
  );

  // Modrinth filters by one loader; Paper also takes Bukkit and Spigot
  // versions, so those are filtered here instead.
  const loaders = compatibleLoaders(target.loader);
  const filter: ModrinthVersionFilter = showAll
    ? {}
    : {
        game_version: target.gameVersion,
        loader: loaders.length === 1 ? loaders[0] : undefined,
      };
  const versionsQuery = useModrinthVersions(project.id, filter);
  const linkedQuery = useModrinthVersion(linkedVersionId);

  const versions = useMemo(() => {
    const list = (versionsQuery.data ?? []).filter(
      (version) =>
        showAll ||
        isVersionCompatible(version, target.loader, target.gameVersion)
    );
    // a version named in the link comes first
    const linked = linkedQuery.data;
    if (linked && linked.project_id === project.id)
      return [linked, ...list.filter((version) => version.id !== linked.id)];
    return list;
  }, [
    versionsQuery.data,
    linkedQuery.data,
    showAll,
    project.id,
    target.loader,
    target.gameVersion,
  ]);

  const installedVersionIds = new Set(
    (target.installed?.mods ?? []).flatMap((mod) =>
      mod.modrinth ? [mod.modrinth.version_id] : []
    )
  );

  const install = async (version: ModrinthVersion, force: boolean) => {
    setInstallingId(version.id);
    try {
      const result = await installMod(target.uuid, {
        version_id: version.id,
        install_dependencies: withDependencies,
        force,
      });
      toast.success(
        <InstallResultMessage
          title={`Installed ${project.title} ${version.version_number}`}
          result={result}
        />
      );
    } catch (error) {
      toast.error(
        `Failed to install ${project.title}: ${errorToString(error)}`
      );
    } finally {
      await invalidateInstalledMods(queryClient, target.uuid);
      setInstallingId(null);
    }
  };

  const requestInstall = (version: ModrinthVersion) => {
    if (isVersionCompatible(version, target.loader, target.gameVersion))
      void install(version, false);
    else setConfirmVersion(version);
  };

  return (
    <div className="flex flex-col gap-3">
      <ConfirmDialog
        title="Install an incompatible version?"
        type="danger"
        isOpen={confirmVersion !== null}
        onClose={() => setConfirmVersion(null)}
        confirmButtonText="Install anyway"
        onConfirm={async () => {
          const version = confirmVersion;
          setConfirmVersion(null);
          if (version) await install(version, true);
        }}
      >
        {confirmVersion && (
          <>
            {project.title} {confirmVersion.version_number} is made for{' '}
            {confirmVersion.loaders.join(', ')}{' '}
            {confirmVersion.game_versions.join(', ')}, not {target.loader}{' '}
            {target.gameVersion}. It may crash the server.
          </>
        )}
      </ConfirmDialog>
      <div className="flex flex-row flex-wrap items-center justify-between gap-2">
        <h2 className="text-h2 font-bold text-gray-300">Versions</h2>
        <div className="flex flex-row flex-wrap items-center gap-4">
          <Checkbox
            label="Also install required dependencies"
            checked={withDependencies}
            onChange={setWithDependencies}
            disabled={!target.canWrite}
          />
          <Checkbox
            label="Show all versions"
            checked={showAll}
            onChange={(checked) => {
              setShowAll(checked);
              setShown(PAGE_SIZE);
            }}
          />
        </div>
      </div>
      {versionsQuery.error ? (
        <div className="text-medium text-red-200">
          Failed to load versions: {errorToString(versionsQuery.error)}
        </div>
      ) : versionsQuery.isPending ? (
        <div className="text-medium text-white/50">Loading versions...</div>
      ) : versions.length === 0 ? (
        <div className="text-medium text-white/50">
          No versions for {target.loader} {target.gameVersion}.{' '}
          {!showAll && 'Turn on "Show all versions" to see the others.'}
        </div>
      ) : (
        <div className="flex flex-col overflow-hidden rounded-lg border border-gray-faded/30 child:border-b child:border-gray-faded/30 child:last:border-b-0">
          {versions.slice(0, shown).map((version) => (
            <VersionRow
              key={version.id}
              version={version}
              target={target}
              installedHere={installedVersionIds.has(version.id)}
              installing={installingId === version.id}
              onInstall={requestInstall}
            />
          ))}
        </div>
      )}
      {versions.length > shown && (
        <Button
          label={`Show more (${versions.length - shown} left)`}
          className="self-center"
          onClick={() => setShown(shown + PAGE_SIZE)}
        />
      )}
    </div>
  );
};

/**
 * A Modrinth project: header, gallery, description, and its versions to
 * install, or for a modpack, a button to create an instance from it.
 */
export default function ModrinthProjectView({
  projectId,
  linkedVersionId,
  author,
  target,
  onBack,
  onCreateFromModpack,
}: {
  projectId: string;
  linkedVersionId: string | null;
  /** from the search result; the project itself has no author */
  author?: string;
  target: InstallTarget;
  onBack: () => void;
  /** null if the user can't create instances */
  onCreateFromModpack:
    ((projectId: string, versionId: string | null) => void) | null;
}) {
  const { data: project, error, isPending } = useModrinthProject(projectId);

  const backButton = (
    <Button
      label="Back"
      icon={faArrowLeft}
      variant="text"
      className="self-start"
      onClick={onBack}
    />
  );

  if (error)
    return (
      <div className="flex flex-col gap-4">
        {backButton}
        <div className="text-medium text-red-200">
          Failed to load the project: {errorToString(error)}
        </div>
      </div>
    );
  if (isPending || !project)
    return (
      <div className="flex flex-col gap-4">
        {backButton}
        <div className="text-medium text-white/50">Loading...</div>
      </div>
    );

  const isModpack = project.project_type === 'modpack';
  const installable =
    project.project_type === 'mod' || project.project_type === 'plugin';
  const noServerSupport = project.server_side === 'unsupported';

  return (
    <div className="flex flex-col gap-6">
      {backButton}
      <div className="flex flex-row items-start gap-4">
        <ModIcon url={project.icon_url} className="size-20" />
        <div className="flex min-w-0 grow flex-col gap-1">
          <h2 className="truncate text-h1 font-bold text-gray-300">
            {project.title}
          </h2>
          <div className="text-medium text-white/50">
            {author ? `by ${author}` : project.slug} · {project.project_type}
          </div>
          <div className="text-medium text-gray-300">{project.description}</div>
          <div className="flex flex-row flex-wrap items-center gap-3 text-small text-white/50">
            <span className="flex items-center gap-1">
              <FontAwesomeIcon icon={faDownload} />
              {formatCount(project.downloads)}
            </span>
            <span className="flex items-center gap-1">
              <FontAwesomeIcon icon={faHeart} />
              {formatCount(project.followers)}
            </span>
            <span className="flex items-center gap-1">
              <FontAwesomeIcon icon={faServer} />
              server: {project.server_side}
            </span>
            {project.categories.map((category) => (
              <Label key={category} size="small" color="gray">
                {category}
              </Label>
            ))}
          </div>
          <ProjectLinks project={project} />
        </div>
      </div>

      <Gallery project={project} />

      {isModpack ? (
        onCreateFromModpack ? (
          <Button
            label="Create an instance from this modpack"
            intention="primary"
            size="large"
            className="self-start"
            onClick={() => onCreateFromModpack(project.id, linkedVersionId)}
          />
        ) : (
          <div className="text-medium text-white/50">
            This is a modpack. You don&apos;t have permission to create
            instances.
          </div>
        )
      ) : installable ? (
        <>
          {noServerSupport && (
            <WarningAlert>
              This project says it doesn&apos;t run on servers.
            </WarningAlert>
          )}
          <ProjectVersions
            project={project}
            linkedVersionId={linkedVersionId}
            target={target}
          />
        </>
      ) : (
        <WarningAlert>
          This is a {project.project_type}. Lodestone only installs{' '}
          {modNoun(target.kind)}s here.
        </WarningAlert>
      )}

      <div className="flex flex-col gap-2">
        <h2 className="text-h2 font-bold text-gray-300">Description</h2>
        <div className="rounded-lg border border-gray-faded/30 bg-gray-800 p-4">
          <SafeMarkdown source={project.body} />
        </div>
      </div>
    </div>
  );
}
