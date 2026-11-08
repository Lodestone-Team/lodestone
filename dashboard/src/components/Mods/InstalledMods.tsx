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
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { Toggle } from 'components/Atoms/Toggle';
import IconButton from 'components/Atoms/IconButton';
import {
  deleteMod,
  installMod,
  invalidateInstalledMods,
  setModEnabled,
  uploadMods,
} from 'data/Mods';
import { useState } from 'react';
import { toast } from 'react-toastify';
import {
  installedModName,
  installedModVersion,
  modNoun,
  modrinthProjectUrl,
  modTabTitle,
} from 'utils/mods';
import { ApiError, errorToString, formatBytes } from 'utils/util';
import {
  ClientRequirementLabel,
  DisabledTooltip,
  InstallResultMessage,
  ModIcon,
  NO_WRITE_PERMISSION,
  pickFiles,
} from './common';
import ClientPackPanel from './ClientPackPanel';

const ModRow = ({
  uuid,
  kind,
  mod,
  canWrite,
  onDelete,
}: {
  uuid: string;
  kind: InstalledModKind;
  mod: InstalledMod;
  canWrite: boolean;
  onDelete: (mod: InstalledMod) => void;
}) => {
  const queryClient = useQueryClient();
  const [toggling, setToggling] = useState(false);
  const [updating, setUpdating] = useState(false);
  const name = installedModName(mod);
  const version = installedModVersion(mod);
  const update = mod.modrinth?.update ?? null;

  const toggle = async (enabled: boolean) => {
    setToggling(true);
    try {
      await setModEnabled(uuid, mod.file_name, enabled);
    } catch (error) {
      toast.error(
        `Failed to ${enabled ? 'enable' : 'disable'} ${name}: ${errorToString(error)}`
      );
    } finally {
      await invalidateInstalledMods(queryClient, uuid);
      setToggling(false);
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
          title={`Updated ${name} to ${update.version_number}`}
          result={result}
        />
      );
    } catch (error) {
      toast.error(`Failed to update ${name}: ${errorToString(error)}`);
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
          {kind === 'mod' && <ClientRequirementLabel mod={mod} />}
        </div>
        <div className="truncate text-small text-white/30">
          {mod.file_name} · {formatBytes(mod.size)}
        </div>
      </div>
      <div className="flex shrink-0 flex-row items-center gap-2">
        {update && (
          <DisabledTooltip show={!canWrite} message={NO_WRITE_PERMISSION}>
            <Button
              label={`Update to ${update.version_number}`}
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
        {mod.modrinth && (
          <a
            href={modrinthProjectUrl(kind, mod.modrinth.slug)}
            target="_blank"
            rel="noreferrer noopener"
            title="View on Modrinth"
            aria-label="View on Modrinth"
            className="rounded-sm p-1.5 text-white/50 hover:text-white/75 focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden"
          >
            <FontAwesomeIcon icon={faArrowUpRightFromSquare} />
          </a>
        )}
        <DisabledTooltip show={!canWrite} message={NO_WRITE_PERMISSION}>
          <Toggle
            value={mod.enabled}
            onChange={toggle}
            disabled={!canWrite || toggling}
          />
        </DisabledTooltip>
        <DisabledTooltip show={!canWrite} message={NO_WRITE_PERMISSION}>
          <IconButton
            icon={faTrashCan}
            disabled={!canWrite}
            onClick={() => onDelete(mod)}
          />
        </DisabledTooltip>
      </div>
    </div>
  );
};

/** The Installed sub-view of the mods tab. */
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
  const queryClient = useQueryClient();
  const [toDelete, setToDelete] = useState<InstalledMod | null>(null);
  // files whose upload hit an existing file, waiting for "Overwrite"
  const [overwriteFiles, setOverwriteFiles] = useState<File[] | null>(null);
  const [overwriteMessage, setOverwriteMessage] = useState('');
  const [uploading, setUploading] = useState(false);
  const [updatingAll, setUpdatingAll] = useState(false);
  const kind = list?.kind ?? 'mod';
  const noun = modNoun(kind);
  const updates = (list?.mods ?? []).filter((mod) => mod.modrinth?.update);

  const upload = async (files: File[], overwrite: boolean) => {
    setUploading(true);
    try {
      const uploaded = await uploadMods(uuid, files, overwrite);
      toast.success(
        `Uploaded ${uploaded.map((mod) => installedModName(mod)).join(', ')}`
      );
      setOverwriteFiles(null);
    } catch (error) {
      if (error instanceof ApiError && error.status === 409 && !overwrite) {
        setOverwriteMessage(error.message);
        setOverwriteFiles(files);
      } else {
        toast.error(`Upload failed: ${errorToString(error)}`);
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
        `Only .jar files can be uploaded: ${notJars.map((f) => f.name).join(', ')}`
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
        <InstallResultMessage title="Updates installed" result={total} />
      );
    if (failures.length > 0)
      toast.error(`Some updates failed: ${failures.join('; ')}`);
    setUpdatingAll(false);
  };

  const confirmDelete = async () => {
    if (!toDelete) return;
    try {
      await deleteMod(uuid, toDelete.file_name);
      toast.success(`Deleted ${toDelete.file_name}`);
      setToDelete(null);
    } catch (error) {
      toast.error(`Failed to delete: ${errorToString(error)}`);
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
        <div className="flex flex-col gap-px overflow-hidden rounded-lg border border-gray-faded/30">
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="h-16 rounded-none" />
          ))}
        </div>
      </SkeletonGroup>
    );

  return (
    <div className="flex flex-col gap-4">
      <ConfirmDialog
        title={`Delete ${noun}?`}
        type="danger"
        isOpen={toDelete !== null}
        onClose={() => setToDelete(null)}
        confirmButtonText="Delete"
        onConfirm={confirmDelete}
      >
        {toDelete && (
          <>
            {installedModName(toDelete)} ({toDelete.file_name}) will be deleted
            from the instance. This can&apos;t be undone.
          </>
        )}
      </ConfirmDialog>
      <ConfirmDialog
        title="Overwrite existing files?"
        type="danger"
        isOpen={overwriteFiles !== null}
        onClose={() => setOverwriteFiles(null)}
        confirmButtonText="Overwrite"
        onConfirm={async () => {
          if (overwriteFiles) await upload(overwriteFiles, true);
        }}
      >
        {overwriteMessage || 'A file with the same name is already installed.'}{' '}
        Replace it with the uploaded file?
      </ConfirmDialog>

      {list && !error && <ClientPackPanel uuid={uuid} list={list} />}
      <div className="flex flex-row flex-wrap items-center justify-between gap-2">
        <div className="text-medium text-white/50">
          {list
            ? `${list.mods.length} ${list.mods.length === 1 ? noun : `${noun}s`} in ${list.directory}/`
            : ' '}
        </div>
        <div className="flex flex-row flex-wrap items-center gap-2">
          {updates.length > 0 && (
            <DisabledTooltip show={!canWrite} message={NO_WRITE_PERMISSION}>
              <Button
                label={`Update all (${updates.length})`}
                icon={faCircleUp}
                intention="primary"
                loading={updatingAll}
                disabled={!canWrite}
                onClick={updateAll}
              />
            </DisabledTooltip>
          )}
          <Button
            label="Check for updates"
            icon={faArrowsRotate}
            loading={checkingUpdates}
            disabled={!list}
            onClick={onCheckUpdates}
          />
          <DisabledTooltip show={!canWrite} message={NO_WRITE_PERMISSION}>
            <Button
              label="Upload"
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
          Failed to load {modTabTitle(kind).toLowerCase()}:{' '}
          {errorToString(error)}
        </div>
      ) : isLoading || !list ? (
        <div className="rounded-lg border border-gray-faded/30 bg-gray-800 p-4 text-medium text-white/50">
          Loading...
        </div>
      ) : list.mods.length === 0 ? (
        <div className="rounded-lg border border-gray-faded/30 bg-gray-800 p-6 text-center text-medium text-white/50">
          No {noun}s installed yet. Browse Modrinth or upload a .jar file.
        </div>
      ) : (
        <div className="flex flex-col overflow-hidden rounded-lg border border-gray-faded/30 child:border-b child:border-gray-faded/30 child:last:border-b-0">
          {list.mods.map((mod) => (
            <ModRow
              key={mod.file_name}
              uuid={uuid}
              kind={kind}
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
