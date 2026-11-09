import { InstanceInfo } from 'bindings/InstanceInfo';
import { useGlobalSettings } from './GlobalSettings';
import { useUserInfo } from './UserInfo';
import { t as translate, useT } from 'i18n';

/** The message the core sends with a 403 when code execution is disabled. */
export const CODE_EXECUTION_DISABLED_MESSAGE =
  'Code execution is disabled on this core';

export const isCodeExecutionDisabledError = (message: string) =>
  message.includes(CODE_EXECUTION_DISABLED_MESSAGE);

/** Why a user who isn't the owner can't control an atom server */
export const ownerOnlyAtomMessage = () => translate('console.ownerOnlyAtom');

/**
 * The core's code execution kill switch and whether the current user may use
 * code execution features (macros, atom instances, extensions, instance
 * scripts, custom Java commands). Docker instances are not one of them.
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

/**
 * A Docker instance: an existing container the core manages through the
 * Docker daemon. The core gives them `DOCKER-<container name>` UUIDs and
 * routes every request for such a UUID to Docker. They run no Lodestone code,
 * so the owner-only and code execution rules don't apply to them.
 */
export const isDockerInstance = (instance: Pick<InstanceInfo, 'uuid'>) =>
  instance.uuid.startsWith('DOCKER-');

/**
 * An atom instance: TypeScript from a URL that the core runs as a macro.
 * Docker instances also report a `Generic` game type, so they are told apart
 * by their UUID. An atom chooses the game type it reports; one that reports
 * another game is not detected here, and the core still enforces the rules.
 */
export const isAtomInstance = (
  instance: Pick<InstanceInfo, 'uuid' | 'game_type'>
) => instance.game_type.type === 'Generic' && !isDockerInstance(instance);

/**
 * Whether the current user may start, stop, configure, rename, delete or
 * send console input to this instance as far as the owner-only rule for
 * atom instances goes. Always true for other instances, including Docker
 * instances; their normal permissions still apply.
 */
export const useCanControlInstance = (
  instance: Pick<InstanceInfo, 'uuid' | 'game_type'> | null | undefined
) => {
  const { canManage } = useCodeExecution();
  return !instance || !isAtomInstance(instance) || canManage;
};

// Settings that choose the program an instance runs and its arguments.
export const COMMAND_LINE_SETTINGS = ['java_cmd', 'cmd_args', 'custom_cmd'];

/**
 * Why the current user can't change `java_cmd` / `cmd_args` / `custom_cmd`, or null if they
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
