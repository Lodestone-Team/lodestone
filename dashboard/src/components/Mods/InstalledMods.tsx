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
import { useState } from 'react';
import { toast } from 'react-toastify';
import {
  installedModName,
  installedModVersion,
  modrinthProjectUrl,
} from 'utils/mods';
import { ApiError, errorToString } from 'utils/util';
import {
  ClientRequirementLabel,
  DisabledTooltip,
  InstallResultMessage,
  ModIcon,
  pickFiles,
  WithTooltip,
} from './common';
import ClientPackPanel from './ClientPackPanel';

const ModRow = ({
  uuid,
  kind,
  marks,
  mod,
  canWrite,
  onDelete,
}: {
  uuid: string;
  kind: InstalledModKind;
  /** the row has "For players" (mods of a Fabric server) */
  marks: boolean;
  mod: InstalledMod;
  canWrite: boolean;
  onDelete: (mod: InstalledMod) => void;
}) => {
  const { t, formatBytes, formatList } = useI18n();
  const queryClient = useQueryClient();
  const [toggling, setToggling] = useState(false);
  const [marking, setMarking] = useState(false);
  const [updating, setUpdating] = useState(false);
  const name = installedModName(mod);
  const version = installedModVersion(mod);
  const update = mod.modrinth?.update ?? null;

  const toggle = async (enabled: boolean) => {
    setToggling(true);
    try {
      await setModEnabled(uuid, mod.file_name, enabled);
    } catch (error) {
      const params = { name, error: errorToString(error) };
      toast.error(
        enabled
          ? translate('mods.installed.enableFailed', params)
          : translate('mods.installed.disableFailed', params)
      );
    } finally {
      await invalidateInstalledMods(queryClient, uuid);
      setToggling(false);
    }
  };

  // players get it whatever the box says: Modrinth or the jar says so, or a
  // mod they get depends on it
  const requiredAnyway =
    mod.client_requirement === 'required' && !mod.client_marked;
  const markTooltip = !canWrite
    ? t('mods.noWritePermission')
    : requiredAnyway && mod.required_by.length > 0
      ? t('mods.clientRequirement.neededBy', {
          mods: formatList(mod.required_by),
        })
      : requiredAnyway
        ? t('mods.forPlayers.already')
        : t('mods.forPlayers.hint');

  const mark = async (required: boolean) => {
    setMarking(true);
    try {
      await setClientMark(uuid, mod.file_name, required);
      toast.success(
        required
          ? translate('mods.forPlayers.marked', { name })
          : translate('mods.forPlayers.unmarked', { name })
      );
    } catch (error) {
      toast.error(
        translate('mods.forPlayers.failed', {
          name,
          error: errorToString(error),
        })
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
        <InstallResultMessage
          title={translate('mods.installed.updated', {
            name,
            version: update.version_number,
          })}
          result={result}
        />
      );
    } catch (error) {
      toast.error(
        translate('mods.installed.updateFailed', {
          name,
          error: errorToString(error),
        })
      );
    } finally {
      await invalidateInstalledMods(queryClient, uuid);
      setUpdating(false);
    }
  };

  return (
    <div className="flex flex-row items-center gap-3 bg-gray-800 px-4 py-3">
      <ModIcon url={mod.modrinth?.icon_url} />
      <div className="flex min-w-0 grow flex-col">
        <div className="flex min-w-0 flex-row items-baseline gap-2">
          <span
            className={
              mod.enabled
                ? 'truncate text-medium font-bold text-gray-300'
                : 'truncate text-medium font-bold text-white/50'
            }
          >
            {name}
          </span>
          {version && (
            <span className="shrink-0 truncate text-small text-white/50">
              {version}
            </span>
          )}
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
        <div className="truncate text-small text-white/30">
          {mod.file_name} · {formatBytes(mod.size)}
        </div>
      </div>
      <div className="flex shrink-0 flex-row items-center gap-2">
        {update && (
          <DisabledTooltip
            show={!canWrite}
            message={t('mods.noWritePermission')}
          >
            <Button
              label={t('mods.installed.updateTo', {
                version: update.version_number,
              })}
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
              checked={mod.client_marked || requiredAnyway}
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
  list,
  isLoading,
  error,
  canWrite,
  checkingUpdates,
  onCheckUpdates,
}: {
  uuid: string;
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
        translate('mods.installed.uploaded', {
          names: uploaded.map((mod) => installedModName(mod)).join(', '),
        })
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
        translate('mods.installed.onlyJars', {
          names: notJars.map((f) => f.name).join(', '),
        })
      );
      return;
    }
    await upload(files, false);
  };

  const updateAll = async () => {
    setUpdatingAll(true);
    const total: ModInstallResult = { installed: [], removed: [], skipped: [] };
    const failures: string[] = [];
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
        failures.push(`${installedModName(mod)}: ${errorToString(error)}`);
      }
    }
    await invalidateInstalledMods(queryClient, uuid);
    if (total.installed.length > 0 || total.removed.length > 0)
      toast.success(
        <InstallResultMessage
          title={translate('mods.installed.updatesInstalled')}
          result={total}
        />
      );
    if (failures.length > 0)
      toast.error(
        translate('mods.installed.someUpdatesFailed', {
          failures: failures.join('; '),
        })
      );
    setUpdatingAll(false);
  };

  const confirmDelete = async () => {
    if (!toDelete) return;
    try {
      await deleteMod(uuid, toDelete.file_name);
      toast.success(
        translate('mods.installed.deleted', { name: toDelete.file_name })
      );
      setToDelete(null);
    } catch (error) {
      toast.error(
        translate('mods.installed.deleteFailed', {
          error: errorToString(error),
        })
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
      >
        {toDelete &&
          t('mods.installed.deleteBody', {
            name: installedModName(toDelete),
            fileName: toDelete.file_name,
          })}
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
        {/* the message comes from the core */}
        {t('mods.installed.overwriteBody', {
          message: overwriteMessage || t('mods.installed.overwriteDefault'),
        })}
      </ConfirmDialog>

      {list && !error && <ClientPackPanel uuid={uuid} list={list} />}
      <div className="flex flex-row flex-wrap items-center justify-between gap-2">
        <div className="text-medium text-white/50">
          {list
            ? isPlugin
              ? t('mods.installed.count.plugin', {
                  count: list.mods.length,
                  directory: list.directory,
                })
              : t('mods.installed.count.mod', {
                  count: list.mods.length,
                  directory: list.directory,
                })
            : ' '}
        </div>
        <div className="flex flex-row flex-wrap items-center gap-2">
          {updates.length > 0 && (
            <DisabledTooltip
              show={!canWrite}
              message={t('mods.noWritePermission')}
            >
              <Button
                label={t('mods.installed.updateAll', {
                  count: updates.length,
                })}
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
        <div className="flex flex-col overflow-hidden rounded-lg border border-gray-faded/30 child:border-b child:border-gray-faded/30 child:last:border-b-0">
          {list.mods.map((mod) => (
            <ModRow
              key={mod.file_name}
              uuid={uuid}
              kind={kind}
              marks={marks}
              mod={mod}
              canWrite={canWrite}
              onDelete={setToDelete}
            />
          ))}
        </div>
      )}
    </div>
  );
}
