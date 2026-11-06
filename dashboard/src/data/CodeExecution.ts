import { InstanceInfo } from 'bindings/InstanceInfo';
import { useGlobalSettings } from './GlobalSettings';
import { useUserInfo } from './UserInfo';

/** The message the core sends with a 403 when code execution is disabled. */
export const CODE_EXECUTION_DISABLED_MESSAGE =
  'Code execution is disabled on this core';

export const isCodeExecutionDisabledError = (message: string) =>
  message.includes(CODE_EXECUTION_DISABLED_MESSAGE);

export const OWNER_ONLY_GENERIC_MESSAGE =
  'Only the owner can control Docker instances';

/**
 * The core's code execution kill switch and whether the current user may use
 * code execution features (macros, atom/Docker instances, extensions,
 * instance scripts, custom Java commands).
 *
 * - `enabled`: the switch is on. A missing field (still loading, or an older
 *   core without the switch) counts as on.
 * - `locked`: the server administrator forced the switch off.
 * - `canManage`: the user is the owner. Macros, extensions and atom
 *   instances are owner-only, whatever permissions a user has.
 * - `canRun`: `canManage` and `enabled`, i.e. the controls that run or
 *   change code can be used now.
 */
export const useCodeExecution = () => {
  const { data: user } = useUserInfo();
  const { data: globalSettings } = useGlobalSettings();
  const locked = globalSettings?.code_execution_locked ?? false;
  const enabled = !locked && (globalSettings?.code_execution_enabled ?? true);
  const canManage = user?.is_owner ?? false;
  return { enabled, locked, canManage, canRun: canManage && enabled };
};

export const isGenericInstance = (instance: Pick<InstanceInfo, 'game_type'>) =>
  instance.game_type.type === 'Generic';

/**
 * Whether the current user may start, stop, configure, rename, delete or
 * send console input to this instance as far as the owner-only rule for
 * atom (Docker) instances goes. Always true for other instances; their normal
 * permissions still apply.
 */
export const useCanControlInstance = (
  instance: Pick<InstanceInfo, 'game_type'> | null | undefined
) => {
  const { canManage } = useCodeExecution();
  return !instance || !isGenericInstance(instance) || canManage;
};

// Settings that choose the program an instance runs and its arguments.
export const COMMAND_LINE_SETTINGS = ['java_cmd', 'cmd_args'];

/**
 * Why the current user can't change `java_cmd` / `cmd_args`, or null if they
 * can. The core allows the owner, or, with safe mode off, a user allowed to
 * write global files (handlers/instance_config.rs), and nobody while code
 * execution is disabled.
 */
export const useCommandLineBlockedReason = (): string | null => {
  const { data: user } = useUserInfo();
  const { data: globalSettings } = useGlobalSettings();
  const { enabled } = useCodeExecution();
  if (!enabled)
    return 'Read-only while code execution is disabled on this core.';
  if (!user) return 'Only the owner can change this setting.';
  if (user.is_owner) return null;
  if (
    user.permissions.can_write_global_file &&
    globalSettings?.safe_mode === false
  )
    return null;
  return 'Only the owner can change this setting.';
};
