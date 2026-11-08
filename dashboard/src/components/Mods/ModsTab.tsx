import { faTriangleExclamation } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import Checkbox from 'components/Atoms/Checkbox';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { InstanceContext } from 'data/InstanceContext';
import { useInstalledMods } from 'data/Mods';
import { useUserAuthorized } from 'data/UserInfo';
import { useContext, useEffect, useMemo, useState } from 'react';
import { useDocumentTitle } from 'usehooks-ts';
import { getModSupport, modNoun, modTabTitle } from 'utils/mods';
import { cn } from 'utils/util';
import InstalledMods from './InstalledMods';
import ModrinthProjectView, { InstallTarget } from './ModrinthProjectView';
import ModrinthSearch, { ModrinthPick, SortSelect } from './ModrinthSearch';

type SubView = 'installed' | 'browse';

/** The Browse sub-view: search, and the project view of a pick. */
const Browse = ({
  target,
  initialLink,
  onCreateFromModpack,
}: {
  target: InstallTarget;
  initialLink: string;
  onCreateFromModpack:
    ((projectId: string, versionId: string | null) => void) | null;
}) => {
  const [onlyCompatible, setOnlyCompatible] = useState(true);
  const [index, setIndex] = useState<ModrinthSearchIndex>('relevance');
  const [picked, setPicked] = useState<ModrinthPick | null>(null);
  const installedProjectIds = useMemo(
    () =>
      new Set(
        (target.installed?.mods ?? []).flatMap((mod) =>
          mod.modrinth ? [mod.modrinth.project_id] : []
        )
      ),
    [target.installed]
  );
  const noun = modNoun(target.kind);

  return (
    <>
      {/* kept mounted, so the search and its page survive the project view */}
      <div className={cn(picked && 'hidden')}>
        <ModrinthSearch
          projectType={target.kind}
          gameVersion={onlyCompatible ? target.gameVersion : undefined}
          loader={onlyCompatible ? target.loader : undefined}
          index={index}
          initialText={initialLink}
          placeholder={`Search ${noun}s on Modrinth, or paste a Modrinth link`}
          installedProjectIds={installedProjectIds}
          showClientSide={target.kind === 'mod'}
          onPick={setPicked}
          toolbar={
            <div className="flex flex-row flex-wrap items-center justify-between gap-3">
              <Checkbox
                label={`Only show versions for ${target.gameVersion} / ${target.loader}`}
                checked={onlyCompatible}
                onChange={setOnlyCompatible}
              />
              <SortSelect value={index} onChange={setIndex} />
            </div>
          }
        />
      </div>
      {picked && (
        <ModrinthProjectView
          key={`${picked.projectId}-${picked.versionId}`}
          projectId={picked.projectId}
          linkedVersionId={picked.versionId}
          author={picked.author}
          target={target}
          onBack={() => setPicked(null)}
          onCreateFromModpack={onCreateFromModpack}
        />
      )}
    </>
  );
};

/**
 * The Mods (or Plugins) tab of an instance: what is installed, and Modrinth
 * browsing. `?modrinth=<link>` opens Browse with that link.
 */
export default function ModsTab() {
  const {
    selectedInstance: instance,
    setShowCreateInstance,
    setModpackPreset,
  } = useContext(InstanceContext);
  if (!instance) throw new Error('No server selected');
  const uuid = instance.uuid;
  const support = getModSupport(instance.game_type);
  useDocumentTitle(
    `Server ${support ? modTabTitle(support.kind) : 'Mods'} - Lodestone`
  );
  const canRead = useUserAuthorized('can_read_instance_file', uuid);
  const canWrite = useUserAuthorized('can_write_instance_file', uuid);
  const canCreate = useUserAuthorized('can_create_instance');
  const { searchParams, setSearchParam } = useContext(BrowserLocationContext);

  // A deep link is read when the tab mounts (InstanceTabs remounts it for
  // every navigation to the tab), and opens Browse with the link searched.
  const [deepLink] = useState(() => searchParams.get('modrinth') ?? '');
  const [view, setView] = useState<SubView>(deepLink ? 'browse' : 'installed');
  // Browse mounts when first opened, then stays mounted to keep its state
  const [browseOpened, setBrowseOpened] = useState(!!deepLink);
  const [checkUpdates, setCheckUpdates] = useState(false);

  const hasModrinthParam = searchParams.has('modrinth');
  useEffect(() => {
    // consumed: switching tabs or reloading must not reopen it
    if (hasModrinthParam) setSearchParam('modrinth', undefined, true);
  }, [hasModrinthParam, setSearchParam]);

  const installedQuery = useInstalledMods(
    uuid,
    checkUpdates,
    canRead && !!support
  );
  const list = installedQuery.data;

  if (!support)
    return (
      <div className="text-medium text-white/50">
        This server does not support mods or plugins. Fabric, Forge, Paper and
        Spigot servers do.
      </div>
    );
  if (!canRead)
    return (
      <div className="text-medium text-white/50">
        You don&apos;t have permission to read this instance&apos;s files.
      </div>
    );

  const target: InstallTarget = {
    uuid,
    kind: list?.kind ?? support.kind,
    loader: list?.loader ?? support.loader,
    gameVersion: list?.game_version ?? instance.version,
    installed: list,
    canWrite,
  };

  const onCreateFromModpack = canCreate
    ? (projectId: string, versionId: string | null) => {
        setModpackPreset({ projectId, versionId });
        setShowCreateInstance(true);
      }
    : null;

  const subViews: Array<[SubView, string]> = [
    ['installed', 'Installed'],
    ['browse', 'Browse'],
  ];

  return (
    <div className="flex flex-col gap-4">
      {list?.restart_required && (
        <div className="flex flex-row items-center gap-2 rounded-md border border-yellow-300 bg-yellow-faded/25 p-2 text-medium font-medium text-white">
          <FontAwesomeIcon
            icon={faTriangleExclamation}
            className="text-yellow-300"
          />
          Restart the server to apply {modNoun(target.kind)} changes
        </div>
      )}
      <div className="flex flex-row justify-start border-b border-gray-400">
        {subViews.map(([id, label]) => (
          <button
            key={id}
            className={cn(
              view === id && 'border-b-2 border-blue-200 text-blue-200',
              'mr-4 font-mediumbold hover:cursor-pointer',
              'focus-visible:outline-hidden enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50'
            )}
            onClick={() => {
              setView(id);
              if (id === 'browse') setBrowseOpened(true);
            }}
          >
            {label}
          </button>
        ))}
      </div>
      <div className={cn(view !== 'installed' && 'hidden')}>
        <InstalledMods
          uuid={uuid}
          list={list}
          isLoading={installedQuery.isPending}
          error={installedQuery.error}
          canWrite={canWrite}
          checkingUpdates={checkUpdates && installedQuery.isFetching}
          onCheckUpdates={() => {
            if (checkUpdates) void installedQuery.refetch();
            else setCheckUpdates(true);
          }}
        />
      </div>
      {browseOpened && (
        <div className={cn(view !== 'browse' && 'hidden')}>
          <Browse
            target={target}
            initialLink={deepLink}
            onCreateFromModpack={onCreateFromModpack}
          />
        </div>
      )}
    </div>
  );
}
