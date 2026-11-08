import SettingField from 'components/SettingField';
import { InstanceContext } from 'data/InstanceContext';
import { useInstanceManifest } from 'data/InstanceManifest';
import { useContext, useState, useEffect, useMemo } from 'react';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { axiosWrapper, errorToString } from 'utils/util';
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
import {
  SettingsCard,
  SettingsEmptyRow,
  SettingsRow,
  SettingsSection,
} from 'components/Settings/SettingsBlocks';
import { SettingsWithToc } from 'components/Settings/SettingsToc';
import { notifySaved } from 'components/Settings/savedToast';

type StartupSetting = 'auto_start' | 'restart_on_crash';

const STARTUP_SETTINGS: Array<{
  key: StartupSetting;
  label: string;
  description: string;
}> = [
  {
    key: 'auto_start',
    label: 'Auto start',
    description: 'Start this server when Lodestone Core starts',
  },
  {
    key: 'restart_on_crash',
    label: 'Restart on crash',
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
      notifySaved();
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setPending({});
    }
  };

  return (
    <SettingsSection
      id="server-settings-automation"
      title="Automation"
      lead="When Lodestone starts this server by itself."
    >
      <SettingsCard>
        {STARTUP_SETTINGS.map(({ key, label, description }) => (
          <SettingsRow
            key={key}
            className="group relative"
            label={label}
            labelId={`startup-setting-${key}`}
            hint={canEdit ? description : disabledReason}
          >
            <Toggle
              value={pending[key] ?? instance[key]}
              onChange={(value) => void change(key, value)}
              disabled={!canEdit || saving}
              aria-labelledby={`startup-setting-${key}`}
            />
          </SettingsRow>
        ))}
      </SettingsCard>
    </SettingsSection>
  );
}

/** The settings sections' shape, while the settings load. */
function InstanceSettingsSkeleton() {
  return (
    <SkeletonGroup className="flex max-w-170 flex-col gap-6.5">
      {[4, 2].map((rows, i) => (
        <div key={i} className="flex flex-col">
          <div className="mb-2.5 flex flex-col gap-1.5">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-3.5 w-64 max-w-full" />
          </div>
          {/* setting rows: name and description, and the control */}
          <div className="flex w-full min-w-0 flex-col divide-y divide-fade-700 rounded-xl border border-fade-700 bg-gray-850">
            {Array.from({ length: rows }, (_, row) => (
              <div
                key={row}
                className="flex min-h-14 flex-row items-center justify-between gap-4 px-4 py-3"
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

      <SettingsWithToc>
        {sections.map((section) => (
          <SettingsSection
            key={section.section_id}
            id={`server-settings-${section.section_id}`}
            title={section.name}
            lead={section.description}
          >
            <SettingsCard>
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
                        SettingOverrides[settingKey]?.descriptionFunc ??
                        undefined
                      }
                    />
                  );
                })
              ) : (
                <SettingsEmptyRow>
                  <span className="block font-mediumbold text-gray-400">
                    Not available at this moment
                  </span>
                  Try to start this server at least once
                </SettingsEmptyRow>
              )}
            </SettingsCard>
          </SettingsSection>
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
        <SettingsSection
          id="server-settings-danger"
          title="Danger zone"
          lead="These settings can cause irreversible damage to your server."
          danger
        >
          <SettingsCard danger>
            <SettingsRow
              className="group relative"
              label="Delete server"
              danger
              hint={
                can_delete_instance
                  ? 'Permanently deletes this server and its data'
                  : canControlInstance
                    ? 'No permission'
                    : OWNER_ONLY_ATOM_MESSAGE
              }
            >
              <Button
                label="Delete"
                intention="danger"
                disabled={!can_delete_instance}
                onClick={() => {
                  setShowDeleteDialog(true);
                }}
              />
            </SettingsRow>
          </SettingsCard>
        </SettingsSection>
      </SettingsWithToc>
    </>
  );
}
