import { InstanceInfo } from 'bindings/InstanceInfo';
import { PublicUser } from 'bindings/PublicUser';
import { UserPermission } from 'bindings/UserPermission';
import clsx from 'clsx';
import MultiSelectGrid from 'components/Atoms/MultiSelectGrid';
import { Toggle } from 'components/Atoms/Toggle';
import {
  NormalPermissions,
  PermissionDescription,
  UnsafePermissions,
} from 'components/Settings/permissionDescriptions';
import { canGrantToApiKey } from 'data/ApiKeys';
import { NO_SCRIPTING_MESSAGE, useCoreCapabilities } from 'data/SystemInfo';

/**
 * The permissions of an API key with a custom scope. The same controls and
 * layout as the user permission editor in the user settings, limited to the
 * permissions the current user has.
 */
export default function ApiKeyScopeEditor({
  value,
  onChange,
  user,
  instances,
  safeMode,
}: {
  value: UserPermission;
  onChange: (permissions: UserPermission) => void;
  /** The current user: a key can't have more than they do */
  user: PublicUser;
  instances: InstanceInfo[];
  safeMode: boolean;
}) {
  const { macros: macrosAvailable } = useCoreCapabilities();
  const instanceUuids = instances.map((instance) => instance.uuid);
  const instanceName = (uuid: string) =>
    instances.find((instance) => instance.uuid === uuid)?.name || uuid;

  const permissionList = (list: PermissionDescription[]) => (
    <div className="flex w-full flex-col gap-8">
      {list.map(({ permission, title, allows }) => {
        const current = value[permission];
        const grantable = Array.isArray(current)
          ? instanceUuids.some((uuid) =>
              canGrantToApiKey(user, permission, uuid)
            )
          : canGrantToApiKey(user, permission);
        return (
          <div
            key={permission}
            className={clsx(
              'flex gap-4',
              typeof current === 'boolean' &&
                'flex-row items-center justify-between',
              Array.isArray(current) && 'flex-col'
            )}
          >
            <div>
              <div
                className={clsx(
                  'text-h3 leading-tight font-bold tracking-medium',
                  grantable ? 'text-gray-300' : 'text-white/50'
                )}
              >
                {title}
              </div>
              <div className="overflow-hidden text-medium font-medium tracking-medium text-ellipsis text-white/50 italic">
                The key {allows}
                {!grantable && " You don't have this permission."}
              </div>
            </div>
            {Array.isArray(current) &&
              (instanceUuids.length === 0 ? (
                <div className="text-medium font-medium tracking-medium text-white/50 italic">
                  There are no servers you can see.
                </div>
              ) : (
                <MultiSelectGrid
                  className="rounded-lg border border-gray-faded/30 bg-gray-800 px-6 py-4"
                  options={instanceUuids}
                  selectedOptions={current}
                  optionLabel={instanceName}
                  optionKey={(uuid) => uuid}
                  optionDisabled={(uuid) =>
                    !canGrantToApiKey(user, permission, uuid)
                  }
                  onChange={(selected) =>
                    onChange({ ...value, [permission]: selected })
                  }
                  disabled={!grantable}
                />
              ))}
            {typeof current === 'boolean' && (
              <Toggle
                value={current}
                onChange={(checked) =>
                  onChange({ ...value, [permission]: checked })
                }
                disabled={!grantable}
              />
            )}
          </div>
        );
      })}
    </div>
  );

  return (
    <div className="@container flex flex-col gap-8">
      {!user.is_owner && (
        <div className="text-medium font-medium tracking-medium text-white/50">
          A key can only have permissions you have yourself. The permissions and
          servers you don&apos;t have are grayed out.
        </div>
      )}
      {permissionList(NormalPermissions)}
      <div className="flex flex-col gap-1">
        <div className="text-h3 leading-tight font-bold tracking-medium text-red-200">
          Unsafe Settings
        </div>
        {safeMode && (
          <div className="text-medium font-medium tracking-medium text-red-200">
            Safe mode is on, so the core refuses these to keys with custom
            access until safe mode is turned off in core settings.
          </div>
        )}
      </div>
      {permissionList(UnsafePermissions)}
      <div>
        <div className="text-h3 leading-tight font-bold tracking-medium text-white/50">
          Macros, Extensions and Atom Servers
        </div>
        <div className="text-medium font-medium tracking-medium text-white/50 italic">
          {macrosAvailable ? (
            <>
              Keys with custom access can&apos;t use macros, install extensions,
              or create or control atom servers, even if they belong to the
              owner. A key with full access can, if you can.
            </>
          ) : (
            NO_SCRIPTING_MESSAGE
          )}
        </div>
      </div>
    </div>
  );
}
