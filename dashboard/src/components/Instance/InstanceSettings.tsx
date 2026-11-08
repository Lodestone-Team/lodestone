import SettingField from 'components/SettingField';
import { InstanceContext } from 'data/InstanceContext';
import { useInstanceManifest } from 'data/InstanceManifest';
import { useContext, useState, useEffect, useMemo } from 'react';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { axiosWrapper, convertUnicode, errorToString } from 'utils/util';
import Button from 'components/Atoms/Button';
import { Toggle } from 'components/Atoms/Toggle';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useUserAuthorized } from 'data/UserInfo';
import {
  OWNER_ONLY_ATOM_MESSAGE,
  useCanControlInstance,
} from 'data/CodeExecution';
import { useQueryClient } from '@tanstack/react-query';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { toast } from 'react-toastify';
import {
  iterateSections,
  SettingFieldObject,
  SectionFieldObject,
} from './InstanceSettingsCreate/SettingObject';
import { SettingOverrides } from './InstanceSettingsCreate/SettingOverrides';
import { PublicListingSetting } from './PublicListingSetting';
import { ResourcePackSetting } from './ResourcePackSetting';
import { useDocumentTitle } from 'usehooks-ts';

type StartupSetting = 'auto_start' | 'restart_on_crash';

const STARTUP_SETTINGS: Array<{
  key: StartupSetting;
  label: string;
  description: string;
}> = [
  {
    key: 'auto_start',
    label: 'Auto Start',
    description: 'Start this server when Lodestone Core starts',
  },
  {
    key: 'restart_on_crash',
    label: 'Restart on Crash',
    description: 'Start this server again if it crashes',
  },
];

/** Toggles for auto start and restart on crash. */
function StartupSettings({
  instance,
  canEdit,
  disabledReason,
}: {
  instance: InstanceInfo;
  canEdit: boolean;
  disabledReason: string;
}) {
  const queryClient = useQueryClient();
  // the value being saved, shown until the instance list has the new value
  const [pending, setPending] = useState<
    Partial<Record<StartupSetting, boolean>>
  >({});
  const saving = Object.keys(pending).length > 0;
  // these can change from the CLI or another dashboard, so fetch them fresh
  useEffect(() => {
    void queryClient.invalidateQueries({ queryKey: ['instances', 'list'] });
  }, [queryClient, instance.uuid]);

  const change = async (key: StartupSetting, value: boolean) => {
    setPending({ [key]: value });
    try {
      await axiosWrapper<void>({
        method: 'PUT',
        url: `/instance/${instance.uuid}/${key}`,
        data: JSON.stringify(value),
        headers: { 'Content-Type': 'application/json' },
      });
      await queryClient.invalidateQueries({ queryKey: ['instances', 'list'] });
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setPending({});
    }
  };

  return (
    <div className="mb-16 flex flex-col gap-4 @4xl:flex-row">
      <div className="w-full shrink-0">
        <h2 className="text-h2 font-extrabold tracking-medium">Automation</h2>
        <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
          When Lodestone starts this server by itself
        </h3>
      </div>
      <div className="w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
        {STARTUP_SETTINGS.map(({ key, label, description }) => (
          <div
            key={key}
            className="group relative flex h-full flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-h3"
          >
            <div className="flex min-w-0 grow flex-col">
              <label className="text-medium font-medium text-gray-300">
                {label}
              </label>
              <div className="overflow-hidden text-medium font-medium tracking-medium text-ellipsis text-white/50">
                {canEdit ? description : disabledReason}
              </div>
            </div>
            <div className="relative flex w-5/12 shrink-0 flex-row items-center justify-end gap-4">
              <Toggle
                value={pending[key] ?? instance[key]}
                onChange={(value) => void change(key, value)}
                disabled={!canEdit || saving}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

/** The settings sections' shape, while the settings load. */
function InstanceSettingsSkeleton() {
  return (
    <SkeletonGroup className="flex flex-col">
      {[4, 2].map((rows, i) => (
        <div key={i} className="mb-16 flex flex-col gap-4 @4xl:flex-row">
          <div className="flex w-full shrink-0 flex-col gap-2">
            <Skeleton className="h-6 w-40" />
            <Skeleton className="h-5 w-72 max-w-full" />
          </div>
          {/* setting rows: name and description, and the control */}
          <div className="flex w-full min-w-0 flex-col divide-y divide-gray-faded/30 overflow-hidden rounded-lg border border-gray-faded/30">
            {Array.from({ length: rows }, (_, row) => (
              <div
                key={row}
                className="flex h-17 flex-row items-center justify-between gap-4 bg-gray-800 px-4"
              >
                <div className="flex grow flex-col gap-2">
                  <Skeleton className="h-3.5 w-36 rounded-sm" />
                  <Skeleton className="h-3 w-56 max-w-full rounded-sm" />
                </div>
                <Skeleton className="h-8 w-5/12 shrink-0" />
              </div>
            ))}
          </div>
        </div>
      ))}
    </SkeletonGroup>
  );
}

export default function InstanceSetting() {
  useDocumentTitle('Server Settings - Lodestone');
  const { selectedInstance: instance, selectInstance } =
    useContext(InstanceContext);
  if (!instance) throw new Error('No server selected');
  const {
    data: manifest,
    isPending: isLoading,
    error,
  } = useInstanceManifest(instance.uuid);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);

  // atom instances can only be deleted by the owner
  const canControlInstance = useCanControlInstance(instance);
  const can_delete_instance =
    useUserAuthorized('can_delete_instance') && canControlInstance;
  const can_access_instance_setting =
    useUserAuthorized('can_access_instance_setting', instance.uuid) &&
    canControlInstance;
  const queryClient = useQueryClient();

  // computed during render: kept in state by an effect, the sections were
  // empty for the first render with the manifest, so the page rendered
  // without them and then jumped when they were inserted above the rest
  const sections = useMemo<SectionFieldObject[]>(
    () => (manifest ? iterateSections(manifest) : []),
    [manifest]
  );

  if (isLoading) {
    // Only the first time: the settings are kept once loaded (see
    // useInstanceManifest), so reopening the page shows them right away
    return <InstanceSettingsSkeleton />;
  }

  return (
    <>
      <ConfirmDialog
        title={`Delete "${instance.name}"`}
        type={'danger'}
        onClose={() => setShowDeleteDialog(false)}
        onConfirm={() => {
          axiosWrapper({
            method: 'DELETE',
            url: `/instance/${instance.uuid}`,
          })
            .then(() => {
              queryClient.invalidateQueries({
                queryKey: ['instances', 'list'],
              });
              selectInstance(null);
            })
            .catch((err) => {
              const err_message = errorToString(err);
              toast.error(err_message);
            })
            .finally(() => {
              setShowDeleteDialog(false);
            });
        }}
        confirmButtonText="I understand, delete this server"
        isOpen={showDeleteDialog}
      >
        <span className="font-bold">This action cannot be undone.</span> This
        instance&#39;s settings, worlds and backups will be permanently deleted.
        Please backup any important data before proceeding.
      </ConfirmDialog>

      {sections.map((section) => (
        <div
          key={section.section_id}
          className="mb-16 flex flex-col gap-4 @4xl:flex-row"
        >
          <div className="w-full shrink-0">
            <h2 className="text-h2 font-extrabold tracking-medium">
              {section.name}
            </h2>
            <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
              {section.description}
            </h3>
          </div>
          <div className="w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
            {Object.keys(section['settings']).length ? (
              Object.keys(section['settings']).map((settingKey: string) => {
                const setting: SettingFieldObject =
                  section['settings'][settingKey];
                return (
                  <SettingField
                    instance={instance}
                    setting={setting}
                    key={settingKey}
                    sectionId={section.section_id}
                    settingId={settingKey}
                    error={error}
                    descriptionFunc={
                      SettingOverrides[settingKey]?.descriptionFunc ?? undefined
                    }
                  />
                );
              })
            ) : (
              <div className="flex h-full w-full flex-col items-center justify-center bg-gray-800 p-4">
                <h2 className="text-h3 font-bold tracking-medium text-white/50">
                  Not available at this moment
                </h2>
                <h2 className="text-medium font-medium tracking-medium text-white/50">
                  Try to start this server at least once
                </h2>
              </div>
            )}
          </div>
        </div>
      ))}
      <StartupSettings
        instance={instance}
        canEdit={can_access_instance_setting}
        disabledReason={
          canControlInstance ? 'No permission' : OWNER_ONLY_ATOM_MESSAGE
        }
      />
      <PublicListingSetting instance={instance} />
      <ResourcePackSetting
        instance={instance}
        canEdit={can_access_instance_setting}
        disabledReason="No permission"
      />
      <div className="mb-16 flex flex-col gap-4 @4xl:flex-row">
        <div className="w-full shrink-0">
          <h2 className="text-h2 font-extrabold tracking-medium">
            Danger Zone
          </h2>
          <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
            These settings can cause irreversible damage to your server!
          </h3>
        </div>
        <div className="w-full min-w-0 rounded-lg border border-red-faded child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
          <div className="group relative flex h-full flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-h3">
            <div className="flex min-w-0 grow flex-col">
              <label className="text-medium font-medium text-gray-300">
                Delete Server
              </label>
              {can_delete_instance ? (
                <div className="overflow-hidden text-medium font-medium tracking-medium text-ellipsis text-white/50">
                  Permanently deletes this server and its data
                </div>
              ) : (
                <div className="overflow-hidden text-medium font-medium tracking-medium text-ellipsis text-white/50">
                  {canControlInstance
                    ? 'No permission'
                    : OWNER_ONLY_ATOM_MESSAGE}
                </div>
              )}
            </div>
            <div className="relative flex w-5/12 shrink-0 flex-row items-center justify-end gap-4">
              <Button
                label="Delete"
                intention="danger"
                disabled={!can_delete_instance}
                onClick={() => {
                  setShowDeleteDialog(true);
                }}
              />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
