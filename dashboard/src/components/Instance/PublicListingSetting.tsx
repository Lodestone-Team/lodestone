import { useQuery, useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { InstancePublicListing } from 'bindings/InstancePublicListing';
import { PublicListingOverview } from 'bindings/PublicListingOverview';
import { PublicScope } from 'bindings/PublicScope';
import { PublicScopeChange } from 'bindings/PublicScopeChange';
import Checkbox from 'components/Atoms/Checkbox';
import { Toggle } from 'components/Atoms/Toggle';
import { useUserInfo } from 'data/UserInfo';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { axiosWrapper, errorToString } from 'utils/util';

export const OWNER_ONLY_PUBLIC_MESSAGE = 'Only the owner can change this';

const SCOPE_FLAGS: Array<{
  key: 'status' | 'address' | 'client_pack';
  label: string;
}> = [
  { key: 'status', label: 'Status and players' },
  { key: 'address', label: 'Address to join' },
  { key: 'client_pack', label: 'Client mods download' },
];

/** What anyone sees of a listed instance, as a sentence. */
function shownText(scope: PublicScope) {
  const shown = [
    'name and version',
    scope.status && 'status and player count',
    scope.address && 'address',
    scope.client_pack && 'client mods',
  ].filter(Boolean);
  const list =
    shown.length > 1
      ? `${shown.slice(0, -1).join(', ')} and ${shown[shown.length - 1]}`
      : shown[0];
  return `Anyone who can reach this core sees its ${list}, without signing in.`;
}

/**
 * The query keys of the public listing, refreshed after a change so Core
 * Settings and Instance Settings agree.
 */
const overviewKey = ['public_listing'];
const instanceKey = (uuid: string) => ['instances', uuid, 'public_listing'];

/** Changes what an instance shows publicly. Only the owner may. */
async function changeScope(uuid: string, change: PublicScopeChange) {
  return axiosWrapper<PublicScope>({
    method: 'PUT',
    url: `/instance/${uuid}/public_listing`,
    data: JSON.stringify(change),
    headers: { 'Content-Type': 'application/json' },
  });
}

/**
 * One instance's "Public" toggle and what it shows. Editable for the owner,
 * read-only for everyone else.
 */
export function PublicScopeRow({
  uuid,
  label,
  scope,
  switchOn,
  canEdit,
}: {
  uuid: string;
  label: string;
  scope: PublicScope;
  switchOn: boolean;
  canEdit: boolean;
}) {
  const queryClient = useQueryClient();
  const [saving, setSaving] = useState(false);

  const change = async (value: PublicScopeChange) => {
    setSaving(true);
    try {
      const updated = await changeScope(uuid, value);
      queryClient.setQueryData<InstancePublicListing>(
        instanceKey(uuid),
        (old) => old && { ...old, ...updated }
      );
      queryClient.setQueryData<PublicListingOverview>(
        overviewKey,
        (old) =>
          old && {
            ...old,
            instances: old.instances.map((i) =>
              i.uuid === uuid ? { ...i, ...updated } : i
            ),
          }
      );
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setSaving(false);
    }
  };

  const description = !canEdit
    ? OWNER_ONLY_PUBLIC_MESSAGE
    : !switchOn && !scope.listed
      ? 'Turn on "List instances publicly" first.'
      : scope.listed
        ? shownText(scope) +
          (switchOn ? '' : ' Hidden while "List instances publicly" is off.')
        : 'Not shown without signing in.';

  return (
    <div className="group relative flex h-full flex-col gap-2 bg-gray-800 px-4 py-3 text-h3">
      <div className="flex flex-row items-center justify-between gap-4">
        <div className="flex min-w-0 grow flex-col">
          <label className="truncate text-medium font-medium text-gray-300">
            {label}
          </label>
          <div className="text-medium font-medium tracking-medium text-white/50">
            {description}
          </div>
        </div>
        <div className="relative flex shrink-0 flex-row items-center justify-end gap-4">
          <p className="text-small font-medium text-white/50 italic">Public</p>
          <Toggle
            value={scope.listed}
            onChange={(value) => void change({ listed: value })}
            // unlisting always works; listing needs the core setting
            disabled={!canEdit || saving || (!switchOn && !scope.listed)}
          />
        </div>
      </div>
      <div className="flex flex-row flex-wrap gap-x-6 gap-y-2 pl-2.5">
        {SCOPE_FLAGS.map(({ key, label }) => (
          <Checkbox
            key={key}
            label={label}
            checked={scope[key]}
            disabled={!canEdit || saving}
            onChange={(value) => void change({ [key]: value })}
          />
        ))}
      </div>
    </div>
  );
}

/**
 * The instances in Core Settings → Public instances, for the owner. Renders
 * nothing on cores without per-instance scopes.
 */
export function PublicInstanceRows({ switchOn }: { switchOn: boolean }) {
  const { data: overview } = useQuery({
    queryKey: overviewKey,
    queryFn: () =>
      axiosWrapper<PublicListingOverview>({
        method: 'GET',
        url: '/public_listing',
      }),
    retry: false,
  });
  if (!overview) return null;
  if (overview.instances.length === 0)
    return (
      <div className="bg-gray-800 px-4 py-3 text-medium font-medium tracking-medium text-white/50">
        No Minecraft Java Edition instances yet.
      </div>
    );
  return (
    <>
      {overview.instances.map((instance) => (
        <PublicScopeRow
          key={instance.uuid}
          uuid={instance.uuid}
          label={instance.name}
          scope={instance}
          switchOn={switchOn}
          canEdit
        />
      ))}
    </>
  );
}

/**
 * Instance Settings → Sharing: whether the instance is public and what it
 * shows, while the owner's "List instances publicly" core setting is on.
 * Lodestone Launcher reads that list to join the server. Renders nothing on
 * cores without it.
 */
export function PublicListingSetting({ instance }: { instance: InstanceInfo }) {
  const { data: userInfo } = useUserInfo();
  const { data: listing } = useQuery({
    queryKey: instanceKey(instance.uuid),
    queryFn: () =>
      axiosWrapper<InstancePublicListing>({
        method: 'GET',
        url: `/instance/${instance.uuid}/public_listing`,
      }),
    enabled: instance.game_type.type === 'MinecraftJava',
    retry: false,
  });
  // cores before scopes have no flags beyond `listed`
  if (!listing || typeof listing.status !== 'boolean') return null;

  return (
    <div className="mb-16 flex flex-col gap-4 @4xl:flex-row">
      <div className="w-full shrink-0">
        <h2 className="text-h2 font-extrabold tracking-medium">Sharing</h2>
        <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
          Who can find this server without signing in
        </h3>
      </div>
      <div className="w-full min-w-0 overflow-hidden rounded-lg border border-gray-faded/30">
        <PublicScopeRow
          uuid={instance.uuid}
          label="Listed publicly"
          scope={listing}
          switchOn={listing.list_instances_publicly}
          canEdit={userInfo?.is_owner ?? false}
        />
      </div>
    </div>
  );
}
