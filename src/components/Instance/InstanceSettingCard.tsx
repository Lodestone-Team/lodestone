import SettingField from 'components/SettingField';
import { InstanceContext } from 'data/InstanceContext';
import { useInstanceManifest } from 'data/InstanceManifest';
import { useContext, useState, useEffect } from 'react';
import { axiosWrapper, convertUnicode, errorToString } from 'utils/util';
import Button from 'components/Atoms/Button';
import { useUserAuthorized } from 'data/UserInfo';
import { useQueryClient } from '@tanstack/react-query';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { toast } from 'react-toastify';
import {
  iterateSections,
  SettingFieldObject,
  SectionFieldObject,
} from './InstanceSettingsCreate/SettingObject';
import { SettingOverrides } from './InstanceSettingsCreate/SettingOverrides';
export default function InstanceSettingCard() {
  const { selectedInstance: instance, selectInstance } =
    useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const {
    data: manifest,
    isLoading,
    error,
  } = useInstanceManifest(instance.uuid);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);

  const can_delete_instance = useUserAuthorized('can_delete_instance');
  const queryClient = useQueryClient();

  const [sections, setSections] = useState<SectionFieldObject[]>([]);
  useEffect(() => {
    if (!manifest) return;
    setSections(iterateSections(manifest));
  }, [manifest]);

  if (isLoading) {
    return <div>Loading...</div>;
    // TODO: show an unobtrusive loading screen, reduce UI flicker
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
              queryClient.invalidateQueries(['instances', 'list']);
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
        confirmButtonText="I understand, delete this instance"
        isOpen={showDeleteDialog}
      >
        <span className="font-bold">This action cannot be undone.</span> This
        instance&#39;s settings, worlds and backups will be permanently deleted.
        Please backup any important data before proceeding.
      </ConfirmDialog>

      <div>
        {sections.map((section) => (
          <div
            key={section.section_id}
            className="mb-16 flex flex-col gap-4 @4xl:flex-row"
          >
            <div className="w-80 shrink-0">
              <h2 className="text-h2 font-extrabold tracking-medium">
                {section.name}
              </h2>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                {section.description}
              </h3>
            </div>
            <div className="w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
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
                <div className="flex h-full w-full flex-col items-center justify-center bg-gray-800 p-4">
                  <h2 className="text-h3 font-bold tracking-medium text-white/50">
                    Not available at this moment
                  </h2>
                  <h2 className="text-medium font-medium tracking-medium text-white/50">
                    Try to start this instance at least once
                  </h2>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
      <div className="mb-16 flex flex-col gap-4 @4xl:flex-row">
        <div className="w-80 shrink-0">
          <h2 className="text-h2 font-extrabold tracking-medium">
            Danger Zone
          </h2>
          <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
            These settings can cause irreversible damage to your server!
          </h3>
        </div>
        <div className="w-full min-w-0 rounded-lg border border-red-faded child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
          <div className="group relative flex h-full flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-h3">
            <div className="flex min-w-0 grow flex-col">
              {can_delete_instance ? (
                <label className="text-medium font-medium text-gray-300">
                  Delete Instance
                </label>
              ) : (
                <label className="text-medium font-medium text-gray-300">
                  Delete Instance
                </label>
              )}
              {can_delete_instance ? (
                <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
                  Permanently deletes this instance and its data
                </div>
              ) : (
                <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
                  No permission
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
