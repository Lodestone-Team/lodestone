import { useQuery, useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { InstancePublicListing } from 'bindings/InstancePublicListing';
import { Toggle } from 'components/Atoms/Toggle';
import { useUserInfo } from 'data/UserInfo';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { axiosWrapper, errorToString } from 'utils/util';

/**
 * "Listed publicly": shows the instance to anyone, without signing in, while
 * the owner's "List instances publicly" core setting is on. Lodestone Launcher
 * reads that list to join the server. Renders nothing on cores without it.
 */
export function PublicListingSetting({
  instance,
  canEdit,
  disabledReason,
}: {
  instance: InstanceInfo;
  canEdit: boolean;
  disabledReason: string;
}) {
  const queryClient = useQueryClient();
  const { data: userInfo } = useUserInfo();
  const [saving, setSaving] = useState(false);
  const queryKey = ['instances', instance.uuid, 'public_listing'];
  const { data: listing } = useQuery({
    queryKey,
    queryFn: () =>
      axiosWrapper<InstancePublicListing>({
        method: 'GET',
        url: `/instance/${instance.uuid}/public_listing`,
      }),
    enabled: canEdit && instance.game_type.type === 'MinecraftJava',
    retry: false,
  });
  if (!listing) return null;

  const switchOn = listing.list_instances_publicly;
  const description = !canEdit
    ? disabledReason
    : !switchOn
      ? userInfo?.is_owner
        ? 'Turn on "List instances publicly" in Core Settings first.'
        : 'The owner has to turn on "List instances publicly" in Core Settings first.'
      : 'Anyone who can reach this core sees its name, version, status and player count, and can download its client mods, without signing in.';

  const change = async (value: boolean) => {
    setSaving(true);
    try {
      await axiosWrapper<void>({
        method: 'PUT',
        url: `/instance/${instance.uuid}/public_listing`,
        data: JSON.stringify(value),
        headers: { 'Content-Type': 'application/json' },
      });
      queryClient.setQueryData<InstancePublicListing>(
        queryKey,
        (old) => old && { ...old, listed: value }
      );
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="mb-16 flex flex-col gap-4 @4xl:flex-row">
      <div className="w-full shrink-0">
        <h2 className="text-h2 font-extrabold tracking-medium">Sharing</h2>
        <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
          Who can find this server without signing in
        </h3>
      </div>
      <div className="w-full min-w-0 rounded-lg border border-gray-faded/30">
        <div className="group relative flex h-full flex-row items-center justify-between gap-4 rounded-lg bg-gray-800 px-4 py-3 text-h3">
          <div className="flex min-w-0 grow flex-col">
            <label className="text-medium font-medium text-gray-300">
              Listed publicly
            </label>
            <div className="text-medium font-medium tracking-medium text-white/50">
              {description}
              {listing.listed &&
                !switchOn &&
                ' It is listed, but hidden while the core setting is off.'}
            </div>
          </div>
          <div className="relative flex w-5/12 shrink-0 flex-row items-center justify-end gap-4">
            <Toggle
              value={listing.listed}
              onChange={(value) => void change(value)}
              // unlisting always works; listing needs the core setting
              disabled={!canEdit || saving || (!switchOn && !listing.listed)}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
