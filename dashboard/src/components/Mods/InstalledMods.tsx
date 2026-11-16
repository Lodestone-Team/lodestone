import {
  faArrowsRotate,
  faArrowUpRightFromSquare,
  faCircleUp,
  faTrashCan,
  faUpload,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { InstalledMod } from 'bindings/InstalledMod';
import { InstalledModList } from 'bindings/InstalledModList';
import { InstalledModKind } from 'bindings/InstalledModKind';
import { ModInstallResult } from 'bindings/ModInstallResult';
import Button from 'components/Atoms/Button';
import Checkbox from 'components/Atoms/Checkbox';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import {
  Skeleton,
  SkeletonGroup,
  SkeletonRows,
} from 'components/Atoms/Skeleton';
import { Toggle } from 'components/Atoms/Toggle';
import IconButton from 'components/Atoms/IconButton';
import {
  deleteMod,
  installMod,
  invalidateInstalledMods,
  setClientMark,
  setModEnabled,
  uploadMods,
} from 'data/Mods';
import { t as translate, useI18n, useT } from 'i18n';
import {
  Bytes,
  CountBadge,
  MonoText,
  MonoValue,
  ObjectChip,
  VersionTag,
} from 'components/Atoms/Values';
import {
  DiffValue,
  Fact,
  FactLine,
  factsTitle,
  ListCell,
  ListHeader,
  NameList,
  ObjectCard,
  ObjectToast,
} from 'components/Values';
import { useState } from 'react';
import { toast } from 'react-toastify';
import {
  installedModName,
  installedModVersion,
  modrinthProjectUrl,
} from 'utils/mods';
import { ApiError, cn, errorToString } from 'utils/util';
import {
  ClientRequirementLabel,
  DisabledTooltip,
  InstallResult,
  ModIcon,
  NeededBy,
  pickFiles,
  WithTooltip,
} from './common';
import ClientPackPanel from './ClientPackPanel';
import ModpackPanel, { blameOf } from './ModpackPanel';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { ModpackHistory } from 'bindings/ModpackHistory';

const ModRow = ({
  uuid,
  kind,
  marks,
  mod,
  canWrite,
  onDelete,
  blame,
}: {
  uuid: string;
  kind: InstalledModKind;
  /** the row has "For players" (mods of a Fabric server) */
  marks: boolean;
  mod: InstalledMod;
  canWrite: boolean;
  onDelete: (mod: InstalledMod) => void;
  /** when the file came, and the history entry it came with */
  blame: { date: number; change: string } | null;
}) => {
  const { t, formatBytes, formatDate } = useI18n();
  const queryClient = useQueryClient();
  const [toggling, setToggling] = useState(false);
  const [marking, setMarking] = useState(false);
  const [updating, setUpdating] = useState(false);
  const name = installedModName(mod);
  const version = installedModVersion(mod);
  const update = mod.modrinth?.update ?? null;
  const chip = <ObjectChip kind={kind} name={name} />;

  const toggle = async (enabled: boolean) => {
    setToggling(true);
    try {
      await setModEnabled(uuid, mod.file_name, enabled);
    } catch (error) {
      toast.error(
        <ObjectToast
          object={chip}
          outcome={
            enabled
              ? translate('mods.toast.enableFailed')
              : translate('mods.toast.disableFailed')
          }
          reason={errorToString(error)}
        />
      );
    } finally {
      await invalidateInstalledMods(queryClient, uuid);
      setToggling(false);
    }
  };

  // players get it whatever the box says: Modrinth or the jar says so, or a
  // mod they get depends on it; an optional mod the owner can leave out
  const forPlayers = mod.client_requirement === 'required';
  const requiredAnyway =
    forPlayers && !mod.client_marked && !mod.client_optional;
  const markTooltip = !canWrite ? (
    t('mods.noWritePermission')
  ) : requiredAnyway && mod.required_by.length > 0 ? (
    <NeededBy mods={mod.required_by} />
  ) : requiredAnyway ? (
    t('mods.forPlayers.already')
  ) : mod.client_optional ? (
    t('mods.forPlayers.optional')
  ) : (
    t('mods.forPlayers.hint')
  );

  const mark = async (required: boolean) => {
    setMarking(true);
    try {
      await setClientMark(uuid, mod.file_name, required);
      toast.success(
        <ObjectToast
          object={chip}
          outcome={
            required
              ? translate('mods.toast.marked')
              : translate('mods.toast.unmarked')
          }
        />
      );
    } catch (error) {
      toast.error(
        <ObjectToast
          object={chip}
          outcome={translate('mods.toast.markFailed')}
          reason={errorToString(error)}
        />
      );
    } finally {
      // the client pack panel and the other rows' requirements change too
      await invalidateInstalledMods(queryClient, uuid);
      setMarking(false);
    }
  };

  const runUpdate = async () => {
    if (!update) return;
    setUpdating(true);
    try {
      const result = await installMod(uuid, {
        version_id: update.version_id,
        install_dependencies: true,
      });
      toast.success(
        <ObjectToast
          object={
            <>
              {chip}
              <VersionTag version={update.version_number} tone="to" />
            </>
          }
          outcome={translate('mods.toast.updated')}
        >
          <InstallResult result={result} />
        </ObjectToast>
      );
    } catch (error) {
      toast.error(
        <ObjectToast
          object={chip}
          outcome={translate('mods.toast.updateFailed')}
          reason={errorToString(error)}
        />
      );
    } finally {
      await invalidateInstalledMods(queryClient, uuid);
      setUpdating(false);
    }
  };

  const size = formatBytes(mod.size);
  const added = blame ? formatDate(blame.date) : null;
  const addedTitle = blame
    ? factsTitle([
        { label: t('values.key.added'), value: added ?? '' },
        // the entry's label comes from the core
        { label: t('values.key.cameWith'), value: blame.change },
      ])
    : undefined;

  return (
    <div className="flex flex-row items-center gap-3 bg-gray-800 px-4 py-3">
      <ModIcon url={mod.modrinth?.icon_url} />
      <div className="flex min-w-0 grow flex-col">
        {/* clipped across only, so the link's focus ring still shows */}
        <div className="flex min-w-0 flex-row items-center gap-1.5 overflow-x-clip">
          <span
            className={cn(
              // the name keeps some room next to an update's two tags
              'min-w-20 truncate text-medium font-bold',
              mod.enabled ? 'text-gray-300' : 'text-white/50'
            )}
          >
            {name}
          </span>
          {version &&
            (update ? (
              <DiffValue
                className="shrink-0"
                from={<VersionTag version={version} tone="from" max={14} />}
                to={
                  <VersionTag
                    version={update.version_number}
                    tone="to"
                    max={14}
                  />
                }
              />
            ) : (
              <VersionTag version={version} />
            ))}
          {/* players never install plugins, so plugins show nothing */}
          {kind === 'mod' && <ClientRequirementLabel mod={mod} marks={marks} />}
          {/* by the name, so the controls on the right line up on every row */}
          {mod.modrinth && (
            <a
              href={modrinthProjectUrl(kind, mod.modrinth.slug)}
              target="_blank"
              rel="noreferrer noopener"
              title={t('mods.installed.viewOnModrinth')}
              aria-label={t('mods.installed.viewOnModrinth')}
              className="-my-1.5 shrink-0 rounded-sm p-1.5 text-white/50 hover:text-white/75 focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden"
            >
              <FontAwesomeIcon icon={faArrowUpRightFromSquare} />
            </a>
          )}
        </div>
        <FactLine
          title={factsTitle([
            { label: t('values.key.file'), value: mod.file_name },
            { label: t('values.key.size'), value: size },
            ...(blame
              ? [
                  { label: t('values.key.added'), value: added ?? '' },
                  { label: t('values.key.cameWith'), value: blame.change },
                ]
              : []),
          ])}
        >
          <MonoText
            text={mod.file_name}
            max={44}
            className="max-w-[60%] text-white/50 @2xl:max-w-full"
          />
          <Fact label={t('values.key.size')} className="@2xl:hidden">
            <Bytes bytes={mod.size} />
          </Fact>
          {blame && (
            <Fact
              label={t('values.key.added')}
              className="text-ink-muted @2xl:hidden"
            >
              {added}
            </Fact>
          )}
        </FactLine>
      </div>
      <div className="flex shrink-0 flex-row items-center gap-2">
        {update && (
          <DisabledTooltip
            show={!canWrite}
            message={t('mods.noWritePermission')}
          >
            <Button
              label={t('mods.installed.update')}
              icon={faCircleUp}
              intention="primary"
              size="slim"
              className="max-w-48"
              loading={updating}
              disabled={!canWrite}
              onClick={runUpdate}
            />
          </DisabledTooltip>
        )}
        {marks && (
          <WithTooltip message={markTooltip}>
            <Checkbox
              label={t('mods.forPlayers.label')}
              className="mr-2 text-small whitespace-nowrap"
              checked={forPlayers}
              onChange={mark}
              disabled={!canWrite || marking || requiredAnyway}
            />
          </WithTooltip>
        )}
        <DisabledTooltip show={!canWrite} message={t('mods.noWritePermission')}>
          <Toggle
            aria-label={name}
            value={mod.enabled}
            onChange={toggle}
            disabled={!canWrite || toggling}
          />
        </DisabledTooltip>
        <DisabledTooltip show={!canWrite} message={t('mods.noWritePermission')}>
          <IconButton
            icon={faTrashCan}
            aria-label={t('mods.installed.deleteLabel', { name })}
            disabled={!canWrite}
            onClick={() => onDelete(mod)}
          />
        </DisabledTooltip>
      </div>
      {/* last, so the columns line up whatever buttons a row has */}
      <ListCell width="size">
        <Bytes bytes={mod.size} />
      </ListCell>
      <ListCell width="date" title={addedTitle}>
        {added ?? ''}
      </ListCell>
    </div>
  );
};

/** The Installed sub-view of the mods tab. */
// installed mod rows: icon, name and version, and the actions at the end
const ModRowsSkeleton = () => (
  <SkeletonRows
    rows={4}
    icon="large"
    secondLine
    trailing
    className="divide-y divide-gray-faded/30 overflow-hidden rounded-lg border border-gray-faded/30"
    rowClassName="h-16 bg-gray-800 px-4"
  />
);

export default function InstalledMods({
  uuid,
  instance,
  history,
  historyError,
  canUploadPack,
  list,
  isLoading,
  error,
  canWrite,
  checkingUpdates,
  onCheckUpdates,
}: {
  uuid: string;
  instance: InstanceInfo;
  /** the modpack history, for a server made from a modpack */
  history: ModpackHistory | undefined;
  historyError: string | null;
  canUploadPack: boolean;
  list: InstalledModList | undefined;
  isLoading: boolean;
  error: ApiError | null;
  canWrite: boolean;
  checkingUpdates: boolean;
  onCheckUpdates: () => void;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const [toDelete, setToDelete] = useState<InstalledMod | null>(null);
  // files whose upload hit an existing file, waiting for "Overwrite"
  const [overwriteFiles, setOverwriteFiles] = useState<File[] | null>(null);
  const [overwriteMessage, setOverwriteMessage] = useState('');
  const [uploading, setUploading] = useState(false);
  const [updatingAll, setUpdatingAll] = useState(false);
  const kind = list?.kind ?? 'mod';
  const isPlugin = kind === 'plugin';
  // only Fabric servers make a client pack, and players never get plugins
  const marks = !isPlugin && list?.loader === 'fabric';
  const updates = (list?.mods ?? []).filter((mod) => mod.modrinth?.update);

  const upload = async (files: File[], overwrite: boolean) => {
    setUploading(true);
    try {
      const uploaded = await uploadMods(uuid, files, overwrite);
      toast.success(
        <ObjectToast outcome={translate('mods.toast.uploaded')}>
          <NameList
            groups={[
              {
                heading: translate(
                  isPlugin ? 'mods.tab.plugins' : 'mods.tab.mods'
                ),
                tone: 'success',
                items: uploaded.map((mod) => ({
                  id: mod.file_name,
                  name: installedModName(mod),
                })),
              },
            ]}
          />
        </ObjectToast>
      );
      setOverwriteFiles(null);
    } catch (error) {
      if (error instanceof ApiError && error.status === 409 && !overwrite) {
        setOverwriteMessage(error.message);
        setOverwriteFiles(files);
      } else {
        toast.error(
          translate('mods.installed.uploadFailed', {
            error: errorToString(error),
          })
        );
        setOverwriteFiles(null);
      }
    } finally {
      await invalidateInstalledMods(queryClient, uuid);
      setUploading(false);
    }
  };

  const chooseAndUpload = async () => {
    const files = await pickFiles('.jar', true);
    if (files.length === 0) return;
    const notJars = files.filter((f) => !f.name.toLowerCase().endsWith('.jar'));
    if (notJars.length > 0) {
      toast.error(
        <ObjectToast outcome={translate('mods.toast.notUploaded')}>
          <NameList
            groups={[
              {
                heading: translate('mods.toast.notUploadedGroup'),
                items: notJars.map((file, i) => ({
                  id: `${i}:${file.name}`,
                  name: (
                    <MonoText
                      text={file.name}
                      max={32}
                      className="text-small"
                    />
                  ),
                })),
              },
            ]}
          />
        </ObjectToast>
      );
      return;
    }
    await upload(files, false);
  };

  const updateAll = async () => {
    setUpdatingAll(true);
    const total: ModInstallResult = { installed: [], removed: [], skipped: [] };
    const failures: { name: string; error: string }[] = [];
    for (const mod of updates) {
      const update = mod.modrinth?.update;
      if (!update) continue;
      try {
        const result = await installMod(uuid, {
          version_id: update.version_id,
          install_dependencies: true,
        });
        total.installed.push(...result.installed);
        total.removed.push(...result.removed);
        total.skipped.push(...result.skipped);
      } catch (error) {
        failures.push({
          name: installedModName(mod),
          error: errorToString(error),
        });
      }
    }
    await invalidateInstalledMods(queryClient, uuid);
    if (total.installed.length > 0 || total.removed.length > 0)
      toast.success(
        <ObjectToast outcome={translate('mods.installed.updatesInstalled')}>
          <InstallResult result={total} />
        </ObjectToast>
      );
    if (failures.length > 0)
      toast.error(
        <ObjectToast outcome={translate('mods.toast.someFailed')}>
          <NameList
            groups={[
              {
                heading: translate('mods.toast.failedGroup'),
                tone: 'danger',
                // the errors come from the core
                items: failures.map((failure, i) => ({
                  id: `${i}:${failure.name}`,
                  name: failure.name,
                  reason: failure.error,
                })),
              },
            ]}
          />
        </ObjectToast>
      );
    setUpdatingAll(false);
  };

  const confirmDelete = async () => {
    if (!toDelete) return;
    const chip = <ObjectChip kind={kind} name={installedModName(toDelete)} />;
    try {
      await deleteMod(uuid, toDelete.file_name);
      toast.success(
        <ObjectToast object={chip} outcome={translate('mods.toast.deleted')}>
          <Fact label={translate('values.key.file')} className="text-small">
            <MonoText text={toDelete.file_name} max={32} />
          </Fact>
        </ObjectToast>
      );
      setToDelete(null);
    } catch (error) {
      toast.error(
        <ObjectToast
          object={chip}
          outcome={translate('mods.toast.deleteFailed')}
          reason={errorToString(error)}
        />
      );
    } finally {
      await invalidateInstalledMods(queryClient, uuid);
    }
  };

  if (isLoading && !list && !error)
    // First load only (the list is kept once loaded). One quiet outline for
    // the whole view, so the toolbar doesn't show first and then get pushed
    // down by the client modpack panel when the list arrives.
    return (
      <SkeletonGroup className="flex flex-col gap-4">
        <div className="flex flex-row items-center justify-between gap-2">
          <Skeleton className="h-5 w-48" />
          <Skeleton className="h-9 w-64" />
        </div>
        <ModRowsSkeleton />
      </SkeletonGroup>
    );

  return (
    <div className="flex flex-col gap-4">
      <ConfirmDialog
        title={
          isPlugin
            ? t('mods.installed.deleteTitle.plugin')
            : t('mods.installed.deleteTitle.mod')
        }
        type="danger"
        isOpen={toDelete !== null}
        onClose={() => setToDelete(null)}
        confirmButtonText={t('common.delete')}
        onConfirm={confirmDelete}
        object={
          toDelete && (
            <ObjectCard
              icon={
                <ModIcon url={toDelete.modrinth?.icon_url} className="size-8" />
              }
              name={installedModName(toDelete)}
              version={installedModVersion(toDelete)}
              facts={
                <FactLine>
                  <Fact label={t('values.key.file')}>
                    <MonoText text={toDelete.file_name} max={36} />
                  </Fact>
                </FactLine>
              }
            />
          )
        }
      >
        {t('mods.installed.deleteBody')}
      </ConfirmDialog>
      <ConfirmDialog
        title={t('mods.installed.overwriteTitle')}
        type="danger"
        isOpen={overwriteFiles !== null}
        onClose={() => setOverwriteFiles(null)}
        confirmButtonText={t('mods.installed.overwrite')}
        onConfirm={async () => {
          if (overwriteFiles) await upload(overwriteFiles, true);
        }}
      >
        <div className="flex flex-col gap-2">
          {t('mods.installed.overwriteBody')}
          {/* the message comes from the core */}
          <span className="text-medium text-white/50">
            {overwriteMessage || t('mods.installed.overwriteDefault')}
          </span>
        </div>
      </ConfirmDialog>

      <ModpackPanel
        instance={instance}
        history={history}
        historyError={historyError}
        canWrite={canWrite}
        canUpload={canUploadPack}
      />
      {list && !error && <ClientPackPanel uuid={uuid} list={list} />}
      <div className="flex flex-row flex-wrap items-center justify-between gap-2">
        {list ? (
          <div className="flex min-w-0 flex-col">
            <div className="flex items-center gap-1.5 text-medium font-bold text-gray-300">
              {isPlugin ? t('mods.tab.plugins') : t('mods.tab.mods')}
              <CountBadge count={list.mods.length} />
            </div>
            <Fact label={t('values.key.folder')} className="text-small">
              <MonoValue>{list.directory}/</MonoValue>
            </Fact>
          </div>
        ) : (
          <div />
        )}
        <div className="flex flex-row flex-wrap items-center gap-2">
          {updates.length > 0 && (
            <DisabledTooltip
              show={!canWrite}
              message={t('mods.noWritePermission')}
            >
              <Button
                label={t('mods.installed.updateAll')}
                badge={<CountBadge count={updates.length} />}
                icon={faCircleUp}
                intention="primary"
                loading={updatingAll}
                disabled={!canWrite}
                onClick={updateAll}
              />
            </DisabledTooltip>
          )}
          <Button
            label={t('mods.installed.checkUpdates')}
            icon={faArrowsRotate}
            loading={checkingUpdates}
            disabled={!list}
            onClick={onCheckUpdates}
          />
          <DisabledTooltip
            show={!canWrite}
            message={t('mods.noWritePermission')}
          >
            <Button
              label={t('mods.installed.upload')}
              icon={faUpload}
              loading={uploading}
              disabled={!canWrite || !list}
              onClick={chooseAndUpload}
            />
          </DisabledTooltip>
        </div>
      </div>

      {error ? (
        <div className="rounded-lg border border-gray-faded/30 bg-gray-800 p-4 text-medium text-red-200">
          {isPlugin
            ? t('mods.installed.loadFailed.plugin', {
                error: errorToString(error),
              })
            : t('mods.installed.loadFailed.mod', {
                error: errorToString(error),
              })}
        </div>
      ) : isLoading || !list ? (
        <SkeletonGroup label={t('mods.installed.loading')}>
          <ModRowsSkeleton />
        </SkeletonGroup>
      ) : list.mods.length === 0 ? (
        <div className="rounded-lg border border-gray-faded/30 bg-gray-800 p-6 text-center text-medium text-white/50">
          {isPlugin
            ? t('mods.installed.empty.plugin')
            : t('mods.installed.empty.mod')}
        </div>
      ) : (
        <div className="@container flex flex-col overflow-hidden rounded-lg border border-gray-faded/30 child:border-b child:border-gray-faded/30 child:last:border-b-0">
          <ListHeader
            className="bg-gray-800 px-4 pt-2"
            columns={[
              { label: t('values.key.size'), width: 'size' },
              { label: t('values.key.added'), width: 'date' },
            ]}
          />
          {list.mods.map((mod) => (
            <ModRow
              key={mod.file_name}
              uuid={uuid}
              kind={kind}
              marks={marks}
              mod={mod}
              canWrite={canWrite}
              onDelete={setToDelete}
              blame={blameOf(history, list.directory, mod.file_name)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
