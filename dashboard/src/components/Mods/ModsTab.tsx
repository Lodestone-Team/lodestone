import { faTriangleExclamation } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import Checkbox from 'components/Atoms/Checkbox';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { InstanceContext } from 'data/InstanceContext';
import { useInstalledMods } from 'data/Mods';
import { useUserAuthorized } from 'data/UserInfo';
import { useT } from 'i18n';
import { useContext, useEffect, useMemo, useState } from 'react';
import { useDocumentTitle } from 'usehooks-ts';
import { getModSupport } from 'utils/mods';
import { cn } from 'utils/util';
import InstalledMods from './InstalledMods';
import ModrinthProjectView, { InstallTarget } from './ModrinthProjectView';
import ModrinthSearch, { ModrinthPick, SortSelect } from './ModrinthSearch';

type SubView = 'installed' | 'browse';

const SUB_VIEWS: Array<[SubView, `mods.view.${SubView}`]> = [
  ['installed', 'mods.view.installed'],
  ['browse', 'mods.view.browse'],
];

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
  const t = useT();
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
  const isPlugin = target.kind === 'plugin';

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
          placeholder={
            isPlugin
              ? t('mods.browse.placeholder.plugin')
              : t('mods.browse.placeholder.mod')
          }
          installedProjectIds={installedProjectIds}
          showClientSide={target.kind === 'mod'}
          onPick={setPicked}
          toolbar={
            <div className="flex flex-row flex-wrap items-center justify-between gap-3">
              <Checkbox
                label={t('mods.browse.onlyCompatible', {
                  gameVersion: target.gameVersion,
                  loader: target.loader,
                })}
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
  const t = useT();
  const {
    selectedInstance: instance,
    setShowCreateInstance,
    setModpackPreset,
  } = useContext(InstanceContext);
  if (!instance) throw new Error('No server selected');
  const uuid = instance.uuid;
  const support = getModSupport(instance.game_type);
  useDocumentTitle(
    t('common.pageTitle', {
      page:
        support?.kind === 'plugin'
          ? t('mods.pageTitle.plugin')
          : t('mods.pageTitle.mod'),
    })
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
      <div className="text-medium text-white/50">{t('mods.unsupported')}</div>
    );
  if (!canRead)
    return (
      <div className="text-medium text-white/50">
        {t('mods.noReadPermission')}
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

  return (
    <div className="flex flex-col gap-4">
      {list?.restart_required && (
        <div className="flex flex-row items-center gap-2 rounded-md border border-yellow-300 bg-yellow-faded/25 p-2 text-medium font-medium text-white">
          <FontAwesomeIcon
            icon={faTriangleExclamation}
            className="shrink-0 text-yellow-300"
          />
          {target.kind === 'plugin'
            ? t('mods.restartRequired.plugin')
            : t('mods.restartRequired.mod')}
        </div>
      )}
      <div className="flex flex-row justify-start border-b border-gray-400">
        {SUB_VIEWS.map(([id, labelKey]) => (
          <button
            key={id}
            className={cn(
              view === id && 'border-b-2 border-blue-200 text-blue-200',
              'mr-4 font-mediumbold whitespace-nowrap hover:cursor-pointer',
              'focus-visible:outline-hidden enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50'
            )}
            onClick={() => {
              setView(id);
              if (id === 'browse') setBrowseOpened(true);
            }}
          >
            {t(labelKey)}
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
