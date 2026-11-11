import { InstanceInfo } from 'bindings/InstanceInfo';
import { useGlobalSettings } from './GlobalSettings';
import { useUserInfo } from './UserInfo';
import { useT } from 'i18n';

/** The message the core sends with a 403 when code execution is disabled. */
export const CODE_EXECUTION_DISABLED_MESSAGE =
  'Code execution is disabled on this core';

export const isCodeExecutionDisabledError = (message: string) =>
  message.includes(CODE_EXECUTION_DISABLED_MESSAGE);

/**
 * The core's code execution kill switch, which now only guards custom Java
 * commands (`java_cmd` / `cmd_args`), and whether the current user is the owner.
 *
 * - `enabled`: the switch is on. A missing field (still loading, or an older
 *   core without the switch) counts as on.
 * - `locked`: the server administrator forced the switch off.
 * - `canManage`: the user is the owner.
 * - `canRun`: `canManage` and `enabled`.
 */
export const useCodeExecution = () => {
  const { data: user } = useUserInfo();
  const { data: globalSettings } = useGlobalSettings();
  const locked = globalSettings?.code_execution_locked ?? false;
  const enabled = !locked && (globalSettings?.code_execution_enabled ?? true);
  const canManage = user?.is_owner ?? false;
  return { enabled, locked, canManage, canRun: canManage && enabled };
};

/**
 * A Docker instance: an existing container the core manages through the
 * Docker daemon. The core gives them `DOCKER-<container name>` UUIDs and
 * routes every request for such a UUID to Docker. They run no Lodestone code,
 * so the owner-only and code execution rules don't apply to them.
 */
export const isDockerInstance = (instance: Pick<InstanceInfo, 'uuid'>) =>
  instance.uuid.startsWith('DOCKER-');

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
  const t = useT();
  if (!enabled) return t('console.commandLine.readOnly');
  if (!user) return t('console.commandLine.ownerOnly');
  if (user.is_owner) return null;
  if (
    user.permissions.can_write_global_file &&
    globalSettings?.safe_mode === false
  )
    return null;
  return t('console.commandLine.ownerOnly');
};
