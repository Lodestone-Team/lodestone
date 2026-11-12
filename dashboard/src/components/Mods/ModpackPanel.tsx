import {
  faChevronDown,
  faChevronRight,
  faCircleUp,
  faCodeCompare,
  faRotate,
  faUpload,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { AvailableUpdate } from 'bindings/AvailableUpdate';
import { ChangeInfo } from 'bindings/ChangeInfo';
import { ChangeKind } from 'bindings/ChangeKind';
import { HistoryEntryInfo } from 'bindings/HistoryEntryInfo';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { ModpackHistory } from 'bindings/ModpackHistory';
import { ModpackUpdatePlan } from 'bindings/ModpackUpdatePlan';
import { UpdateSource } from 'bindings/UpdateSource';
import Button from 'components/Atoms/Button';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import SafeMarkdown from 'components/SafeMarkdown';
import {
  checkModpack,
  invalidateInstalledMods,
  modpackHistoryKey,
  planModpackUpdate,
  updateModpack,
  uploadModpack,
} from 'data/Mods';
import { t as translate, useI18n } from 'i18n';
import { useRef, useState } from 'react';
import { toast } from 'react-toastify';
import { cn, errorToString } from 'utils/util';

/*
 * The modpack of a server made from one (Mods tab, above the mods): which
 * version it runs, whether a newer one is out (the core checks on its own, and
 * on "Check for an update"), an update from Modrinth or an .mrpack file, and
 * the history: a timeline of every install, update and restore, with what each
 * changed and a button to go back to it. The core works out what an update or
 * restore changes first; the dialog shows that and runs it once confirmed.
 */

/** How many changes an entry lists before "and N more". */
const CHANGES_SHOWN = 40;

const LOADER_TITLES: Record<string, string> = {
  fabric: 'Fabric',
  forge: 'Forge',
  neoforge: 'NeoForge',
};

const loaderTitle = (loader: string) => LOADER_TITLES[loader] ?? loader;

/** The sign and color of a change: + added, ↑ updated, ~ replaced, − removed. */
const CHANGE_MARKS: Record<ChangeKind, [string, string]> = {
  added: ['+', 'text-green'],
  updated: ['↑', 'text-blue-200'],
  replaced: ['~', 'text-yellow'],
  removed: ['−', 'text-red-200'],
};

const CHANGE_ORDER: ChangeKind[] = ['added', 'updated', 'replaced', 'removed'];

const fileName = (path: string) => path.split('/').pop() ?? path;

const shortList = (paths: string[], shown = 8) =>
  paths.slice(0, shown).join(', ') + (paths.length > shown ? ', …' : '');

const entryVersion = (entry: HistoryEntryInfo) =>
  entry.version_number ?? translate('mods.modpack.unknownVersion');

/** "since 6.4.0" for a file of the mods folder: the version it came with. */
export const blameLabel = (
  history: ModpackHistory | undefined,
  directory: string,
  file: string
): { label: string; tooltip: string } | null => {
  // a server that never changed its pack has nothing to tell apart
  if (!history || history.entries.length < 2) return null;
  const enabled = file.toLowerCase().endsWith('.jar.disabled')
    ? file.slice(0, -'.disabled'.length)
    : file;
  const id = history.blame[`${directory}/${enabled}`];
  const entry = history.entries.find((e) => e.id === id);
  if (!entry) return null;
  const version = entryVersion(entry);
  return {
    label: translate('mods.modpack.since', { version }),
    tooltip: translate('mods.modpack.sinceTooltip', { version }),
  };
};

/** "What's new": a quiet link that shows a changelog only when asked. */
const WhatsNewToggle = ({
  open,
  onToggle,
}: {
  open: boolean;
  onToggle: () => void;
}) => {
  const { t } = useI18n();
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={open}
      className="flex flex-row items-center gap-1 text-small text-blue-200 hover:underline"
    >
      <FontAwesomeIcon
        icon={open ? faChevronDown : faChevronRight}
        className="w-2"
      />
      {t('mods.modpack.whatsNew')}
    </button>
  );
};

/** A changelog (Markdown from Modrinth), scrolling when it is long. */
const ChangelogBox = ({ changelog }: { changelog: string }) => (
  <div className="max-h-80 overflow-y-auto rounded-sm bg-gray-900 p-2.5 text-small">
    <SafeMarkdown source={changelog} />
  </div>
);

const CountChips = ({ entry }: { entry: HistoryEntryInfo }) => (
  <>
    {CHANGE_ORDER.map((kind) => {
      const count = entry.changes.filter((c) => c.kind === kind).length;
      if (count === 0) return null;
      const [sign, color] = CHANGE_MARKS[kind];
      return (
        <span
          key={kind}
          className={cn(
            'rounded-xs border border-current/40 px-1.5 text-caption font-bold',
            color
          )}
        >
          {sign}
          {count}
        </span>
      );
    })}
  </>
);

const ChangeList = ({ entry }: { entry: HistoryEntryInfo }) => {
  const { t } = useI18n();
  const line = (change: ChangeInfo, index: number) => {
    const [sign, color] = CHANGE_MARKS[change.kind];
    return (
      <div key={index} className="flex flex-row gap-2">
        <span className={cn('w-3 shrink-0 font-bold', color)}>{sign}</span>
        <span className="truncate text-white/75">
          {change.old_path
            ? `${fileName(change.old_path)} → ${fileName(change.path)}`
            : fileName(change.path)}
        </span>
      </div>
    );
  };
  return (
    <div className="mt-1 flex flex-col gap-0.5 rounded-sm bg-gray-900 p-2 font-mono text-small">
      {entry.changes.slice(0, CHANGES_SHOWN).map(line)}
      {entry.changes.length > CHANGES_SHOWN && (
        <div className="text-white/50">
          {t('mods.modpack.moreChanges', {
            count: entry.changes.length - CHANGES_SHOWN,
          })}
        </div>
      )}
      {entry.kept.length > 0 && (
        <div className="text-white/50">
          {t('mods.modpack.dialog.kept', { files: shortList(entry.kept) })}
        </div>
      )}
    </div>
  );
};

const HistoryRow = ({
  history,
  entry,
  last,
  canWrite,
  busy,
  onRestore,
}: {
  history: ModpackHistory;
  entry: HistoryEntryInfo;
  last: boolean;
  canWrite: boolean;
  busy: boolean;
  onRestore: (entryId: number) => void;
}) => {
  const { t, formatDateTime } = useI18n();
  const [open, setOpen] = useState(entry.current);
  const [changelogOpen, setChangelogOpen] = useState(false);
  const hasChanges =
    entry.changes.length > 0 ||
    entry.kept.length > 0 ||
    entry.changelog !== null;
  const kind =
    entry.kind.type === 'install'
      ? t('mods.modpack.kind.install')
      : entry.kind.type === 'baseline'
        ? t('mods.modpack.kind.baseline')
        : entry.kind.type === 'restore'
          ? t('mods.modpack.kind.restore', {
              version: (() => {
                const to = entry.kind.to;
                const target = history.entries.find((e) => e.id === to);
                return target
                  ? entryVersion(target)
                  : t('mods.modpack.unknownVersion');
              })(),
            })
          : t('mods.modpack.kind.update');
  const missing = entry.missing > 0;
  return (
    <div className="flex flex-row gap-2.5">
      {/* the dot, and the line down to the next entry */}
      <div className="flex w-4 shrink-0 flex-col items-center">
        <div
          className={cn(
            'mt-1.5 size-2.5 rounded-full border-2',
            entry.current ? 'border-green bg-green' : 'border-white/30'
          )}
        />
        {!last && <div className="w-0.5 grow bg-gray-faded/30" />}
      </div>
      <div className="flex min-w-0 grow flex-col gap-1 pb-3.5">
        <div className="flex flex-row items-start justify-between gap-2">
          <div className="flex min-w-0 flex-col gap-0.5">
            <button
              type="button"
              disabled={!hasChanges}
              onClick={() => setOpen(!open)}
              className="flex flex-row flex-wrap items-center gap-2 text-left disabled:cursor-default"
            >
              {hasChanges && (
                <FontAwesomeIcon
                  icon={open ? faChevronDown : faChevronRight}
                  className="w-2.5 text-white/50"
                />
              )}
              <span className="font-bold text-gray-300">
                {entryVersion(entry)}
              </span>
              <span className="text-white/75">{kind}</span>
              {entry.current && (
                <span className="rounded-xs border border-green/40 px-1.5 text-caption font-bold text-green">
                  {t('mods.modpack.current')}
                </span>
              )}
              <CountChips entry={entry} />
            </button>
            <div className="flex flex-row flex-wrap gap-2 text-small text-white/30">
              <span>{formatDateTime(entry.time)}</span>
              <span>
                Minecraft {entry.game_version} · {loaderTitle(entry.loader)}{' '}
                {entry.loader_version ?? ''}
              </span>
              {entry.kept.length > 0 && (
                <span>
                  {t('mods.modpack.keptCount', { count: entry.kept.length })}
                </span>
              )}
            </div>
          </div>
          {!entry.current && (
            <span
              title={missing ? t('mods.modpack.restoreMissing') : undefined}
            >
              <Button
                label={t('mods.modpack.restore')}
                icon={faRotate}
                disabled={!canWrite || missing || busy}
                onClick={() => onRestore(entry.id)}
              />
            </span>
          )}
        </div>
        {open && (entry.changes.length > 0 || entry.kept.length > 0) && (
          <ChangeList entry={entry} />
        )}
        {open && entry.changelog && (
          <div className="mt-1 flex flex-col gap-1">
            <WhatsNewToggle
              open={changelogOpen}
              onToggle={() => setChangelogOpen(!changelogOpen)}
            />
            {changelogOpen && <ChangelogBox changelog={entry.changelog} />}
          </div>
        )}
      </div>
    </div>
  );
};

const PlanBody = ({
  plan,
  current,
  stopped,
}: {
  plan: ModpackUpdatePlan;
  current: string;
  stopped: boolean;
}) => {
  const { t } = useI18n();
  const target = plan.target;
  const counts = (
    [
      ['mods.modpack.added', plan.added.length],
      ['mods.modpack.replaced', plan.replaced.length],
      ['mods.modpack.removed', plan.removed.length],
    ] as const
  )
    .filter(([, count]) => count > 0)
    .map(([key, count]) => t(key, { count }));
  return (
    <span className="flex flex-col gap-2 text-medium text-white/75">
      <span>
        {t('mods.modpack.dialog.intro', {
          pack: target.name,
          from: current,
          to: target.version_number ?? t('mods.modpack.unknownVersion'),
        })}
      </span>
      {target.unsupported_reason && (
        <span className="text-red-200">
          {t('mods.modpack.dialog.unsupported', {
            reason: target.unsupported_reason,
          })}
        </span>
      )}
      {plan.reinstalls_server && (
        <span>
          {t('mods.modpack.dialog.reinstall', {
            software: `${loaderTitle(target.loader)} ${
              target.loader_version ?? ''
            } · Minecraft ${target.game_version}`,
          })}
        </span>
      )}
      <span>
        {counts.length > 0
          ? counts.join(', ')
          : t('mods.modpack.dialog.nothing')}
      </span>
      {plan.removed.length > 0 && (
        <span className="text-white/50">{shortList(plan.removed)}</span>
      )}
      {plan.kept.length > 0 && (
        <span className="text-white/50">
          {t('mods.modpack.dialog.kept', { files: shortList(plan.kept) })}
        </span>
      )}
      {plan.note && <span className="text-white/50">{plan.note}</span>}
      {!stopped && (
        <span className="text-yellow">{t('mods.modpack.dialog.stop')}</span>
      )}
    </span>
  );
};

export default function ModpackPanel({
  instance,
  history,
  historyError,
  canWrite,
  canUpload,
  initialHistoryOpen = false,
}: {
  instance: InstanceInfo;
  history: ModpackHistory | undefined;
  historyError: string | null;
  canWrite: boolean;
  /** uploading a pack needs the permissions to create servers and add jars */
  canUpload: boolean;
  /** the timeline starts open (for stories) */
  initialHistoryOpen?: boolean;
}) {
  const { t, formatBytes } = useI18n();
  const queryClient = useQueryClient();
  const origin = instance.modpack;
  const uuid = instance.uuid;
  const [checking, setChecking] = useState(false);
  // the result of "Check for an update"; until then, the core's own check
  const [checked, setChecked] = useState<
    { update: AvailableUpdate | null } | { error: string } | null
  >(null);
  const [historyOpen, setHistoryOpen] = useState(initialHistoryOpen);
  const [updateChangelogOpen, setUpdateChangelogOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<{
    source: UpdateSource;
    plan: ModpackUpdatePlan;
  } | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  if (!origin) return null;

  const onModrinth = origin.project_id !== '';
  const update =
    checked && 'update' in checked ? checked.update : (history?.update ?? null);
  const stopped = instance.state === 'Stopped';

  const check = async () => {
    setChecking(true);
    try {
      setChecked({ update: await checkModpack(uuid) });
    } catch (error) {
      setChecked({ error: errorToString(error) });
    } finally {
      setChecking(false);
    }
  };

  const plan = async (source: UpdateSource) => {
    setBusy(true);
    try {
      setPending({ source, plan: await planModpackUpdate(uuid, source) });
    } catch (error) {
      toast.error(
        translate('mods.modpack.planFailed', { error: errorToString(error) })
      );
    } finally {
      setBusy(false);
    }
  };

  const uploadFile = async (file: File) => {
    if (!file.name.toLowerCase().endsWith('.mrpack')) {
      toast.error(translate('mods.modpack.notMrpack', { name: file.name }));
      return;
    }
    setBusy(true);
    try {
      const info = await uploadModpack(file);
      if (info.unsupported_reason) {
        toast.error(
          translate('mods.modpack.dialog.unsupported', {
            reason: info.unsupported_reason,
          })
        );
      } else if (info.upload_id) {
        setBusy(false);
        await plan({ type: 'upload', upload_id: info.upload_id });
      }
    } catch (error) {
      toast.error(
        translate('mods.modpack.planFailed', { error: errorToString(error) })
      );
    } finally {
      setBusy(false);
    }
  };

  const run = async () => {
    if (!pending) return;
    const restore = pending.source.type === 'history';
    const target = pending.plan.target;
    const version =
      target.version_number ?? translate('mods.modpack.unknownVersion');
    try {
      const done = await updateModpack(uuid, pending.source);
      const counts = (
        [
          ['mods.modpack.added', done.added],
          ['mods.modpack.replaced', done.replaced],
          ['mods.modpack.removed', done.removed],
        ] as const
      )
        .filter(([, count]) => count > 0)
        .map(([key, count]) => translate(key, { count }));
      toast.success(
        [
          translate(
            restore ? 'mods.modpack.restored' : 'mods.modpack.updated',
            {
              pack: target.name,
              version,
            }
          ),
          counts.join(', '),
          done.skipped ?? '',
        ]
          .filter(Boolean)
          .join('\n')
      );
      setChecked(null);
      setPending(null);
    } catch (error) {
      toast.error(
        translate(
          restore ? 'mods.modpack.restoreFailed' : 'mods.modpack.updateFailed',
          { error: errorToString(error) }
        )
      );
      setPending(null);
    } finally {
      queryClient.invalidateQueries({ queryKey: modpackHistoryKey(uuid) });
      queryClient.invalidateQueries({ queryKey: ['instances', 'list'] });
      invalidateInstalledMods(queryClient, uuid);
    }
  };

  const mayRun =
    pending !== null &&
    stopped &&
    canWrite &&
    !pending.plan.target.unsupported_reason;
  const restoring = pending?.source.type === 'history';
  const label = origin.version_number
    ? `${origin.name} ${origin.version_number}`
    : origin.name;

  return (
    <div className="flex flex-col gap-4 rounded-lg border border-gray-faded/30 bg-gray-800 p-4">
      <ConfirmDialog
        title={
          restoring
            ? t('mods.modpack.restoreTitle')
            : t('mods.modpack.updateTitle')
        }
        type="info"
        isOpen={pending !== null}
        onClose={() => setPending(null)}
        confirmButtonText={
          restoring ? t('mods.modpack.restore') : t('mods.modpack.update')
        }
        onConfirm={mayRun ? run : undefined}
      >
        {pending && (
          <PlanBody
            plan={pending.plan}
            current={origin.version_number ?? t('mods.modpack.unknownVersion')}
            stopped={stopped}
          />
        )}
      </ConfirmDialog>
      <input
        ref={fileInput}
        type="file"
        accept=".mrpack"
        className="hidden"
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = '';
          if (file) uploadFile(file);
        }}
      />

      <div className="flex flex-row flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-1">
          <div className="text-h3 font-bold text-gray-300">{label}</div>
          <div className="text-medium text-white/50">
            {t('mods.modpack.from', { pack: origin.name })}
          </div>
          {update ? (
            <div className="flex flex-row items-center gap-1.5 text-medium text-green">
              <span className="size-1.5 rounded-full bg-green" />
              {t('mods.modpack.out', { version: update.version_number })}
              {update.changelog && (
                <WhatsNewToggle
                  open={updateChangelogOpen}
                  onToggle={() => setUpdateChangelogOpen(!updateChangelogOpen)}
                />
              )}
            </div>
          ) : checked && 'error' in checked ? (
            <div className="text-medium text-red-200">
              {t('mods.modpack.checkFailed', { error: checked.error })}
            </div>
          ) : (
            checked && (
              <div className="text-medium text-white/50">
                {t('mods.modpack.upToDate')}
              </div>
            )
          )}
        </div>
        <div className="flex flex-row flex-wrap items-center gap-2">
          {onModrinth &&
            (update ? (
              <Button
                label={t('mods.modpack.updateTo', {
                  version: update.version_number,
                })}
                icon={faCircleUp}
                intention="primary"
                loading={busy}
                disabled={!canWrite || busy}
                onClick={() =>
                  plan({ type: 'modrinth', version_id: update.version_id })
                }
              />
            ) : (
              <Button
                label={t('mods.modpack.check')}
                loading={checking}
                onClick={check}
              />
            ))}
          <Button
            label={t('mods.modpack.fromFile')}
            icon={faUpload}
            disabled={!canUpload || busy}
            onClick={() => fileInput.current?.click()}
          />
          <Button
            label={
              history && history.entries.length > 0
                ? `${t('mods.modpack.history')} (${history.entries.length})`
                : t('mods.modpack.history')
            }
            icon={faCodeCompare}
            onClick={() => setHistoryOpen(!historyOpen)}
          />
        </div>
      </div>

      {update?.changelog && updateChangelogOpen && (
        <ChangelogBox changelog={update.changelog} />
      )}
      {historyOpen && (
        <div className="flex flex-col border-t border-gray-faded/30 pt-3">
          {historyError ? (
            <div className="text-medium text-red-200">
              {t('mods.modpack.historyFailed', { error: historyError })}
            </div>
          ) : !history ? null : history.entries.length === 0 ? (
            <div className="text-medium text-white/50">
              {t('mods.modpack.historyEmpty')}
            </div>
          ) : (
            <>
              {history.entries.map((entry, index) => (
                <HistoryRow
                  key={entry.id}
                  history={history}
                  entry={entry}
                  last={index === history.entries.length - 1}
                  canWrite={canWrite}
                  busy={busy}
                  onRestore={(entryId) =>
                    plan({ type: 'history', entry_id: entryId })
                  }
                />
              ))}
              {history.stored_bytes > 0 && (
                <div className="pt-2 text-small text-white/30">
                  {t('mods.modpack.storeSize', {
                    size: formatBytes(history.stored_bytes),
                  })}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
