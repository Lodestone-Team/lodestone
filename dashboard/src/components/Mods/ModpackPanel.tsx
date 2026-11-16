import {
  faArrowUp,
  faBoxOpen,
  faChevronDown,
  faChevronRight,
  faCircleUp,
  faClock,
  faCodeCompare,
  faFile,
  faMinus,
  faPlus,
  faRightLeft,
  faRotate,
  faUpload,
  IconDefinition,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { AvailableUpdate } from 'bindings/AvailableUpdate';
import { ChangeInfo } from 'bindings/ChangeInfo';
import { ChangeKind } from 'bindings/ChangeKind';
import { HistoryEntryInfo } from 'bindings/HistoryEntryInfo';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { ModpackHistory } from 'bindings/ModpackHistory';
import { ModpackOrigin } from 'bindings/ModpackOrigin';
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
import {
  CountBadge,
  MonoText,
  Num,
  ObjectChip,
  StatusBadge,
  VersionTag,
} from 'components/Atoms/Values';
import {
  DiffValue,
  Fact,
  FactLine,
  FactList,
  FactRow,
  FactStrip,
  factsTitle,
  NameList,
  ObjectCard,
  ObjectToast,
} from 'components/Values';
import { ReactNode, useRef, useState } from 'react';
import { toast } from 'react-toastify';
import { cn, errorToString } from 'utils/util';
import { gamePhrase, loaderPhrase } from 'utils/values';
import { modpackIconUrl } from 'utils/mods';

/*
 * The modpack of a server made from one (Mods tab, above the mods): which
 * version it runs, whether a newer one is out (the core checks on its own, and
 * on "Check for an update"), an update from Modrinth or an .mrpack file, and
 * the history: a timeline of the server's content images (every install,
 * update and restore, and every change to its mods and configs), with what each
 * changed and a button to go back to it. The core works out what an update or
 * restore changes first; the dialog shows that and runs it once confirmed.
 */

/** How many changes an entry lists before "and N more". */
const CHANGES_SHOWN = 40;

/** The icon, colour and key of each kind of change. */
const CHANGE_MARKS: Record<
  ChangeKind,
  [IconDefinition, string, `mods.modpack.count.${ChangeKind}`]
> = {
  added: [faPlus, 'text-green', 'mods.modpack.count.added'],
  updated: [faArrowUp, 'text-blue-200', 'mods.modpack.count.updated'],
  replaced: [faRightLeft, 'text-yellow', 'mods.modpack.count.replaced'],
  removed: [faMinus, 'text-red-200', 'mods.modpack.count.removed'],
};

const CHANGE_ORDER: ChangeKind[] = ['added', 'updated', 'replaced', 'removed'];

const fileName = (path: string) => path.split('/').pop() ?? path;

/** The modpack's icon (a Modrinth CDN URL), or the modpack kind's icon. */
const PackIcon = ({ url, size }: { url: string | null; size: string }) => {
  const src = modpackIconUrl(url);
  return src ? (
    <img src={src} alt="" className={cn(size, 'rounded-md object-cover')} />
  ) : (
    <FontAwesomeIcon icon={faBoxOpen} className="w-4 text-gray-500" />
  );
};

/** When a file of the mods folder came, and with which history entry. */
export const blameOf = (
  history: ModpackHistory | undefined,
  directory: string,
  file: string
): { date: number; change: string } | null => {
  // a server that never changed its pack has nothing to tell apart
  if (!history || history.entries.length < 2) return null;
  const enabled = file.toLowerCase().endsWith('.jar.disabled')
    ? file.slice(0, -'.disabled'.length)
    : file;
  const id = history.blame[`${directory}/${enabled}`];
  const entry = history.entries.find((e) => e.id === id);
  if (!entry) return null;
  return { date: entry.time, change: entry.label };
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

/** How many files of each kind an entry changed, as icon facts */
const CountChips = ({ entry }: { entry: HistoryEntryInfo }) => {
  const { t, formatNumber } = useI18n();
  return (
    <>
      {CHANGE_ORDER.map((kind) => {
        const count = entry.changes.filter((c) => c.kind === kind).length;
        if (count === 0) return null;
        const [icon, color, key] = CHANGE_MARKS[kind];
        const label = t(key);
        return (
          <Fact
            key={kind}
            label={label}
            icon={icon}
            title={`${label}: ${formatNumber(count)}`}
            className={cn('text-small [&_svg]:text-current', color)}
          >
            <span className="font-bold tabular-nums">
              {formatNumber(count)}
            </span>
          </Fact>
        );
      })}
    </>
  );
};

const ChangeList = ({ entry }: { entry: HistoryEntryInfo }) => {
  const { t } = useI18n();
  const line = (change: ChangeInfo, index: number) => {
    const [icon, color, key] = CHANGE_MARKS[change.kind];
    return (
      <div key={index} className="flex min-w-0 flex-row items-center gap-2">
        <FontAwesomeIcon
          icon={icon}
          className={cn('w-2.5 shrink-0', color)}
          aria-hidden="true"
        />
        <span className="sr-only">{t(key)}: </span>
        <span className="min-w-0 truncate text-white/75">
          {change.old_path ? (
            <DiffValue
              from={<MonoText text={fileName(change.old_path)} max={36} />}
              to={<MonoText text={fileName(change.path)} max={36} />}
            />
          ) : (
            <MonoText text={fileName(change.path)} max={60} />
          )}
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
    </div>
  );
};

const HistoryRow = ({
  entry,
  last,
  canWrite,
  busy,
  onRestore,
}: {
  entry: HistoryEntryInfo;
  last: boolean;
  canWrite: boolean;
  busy: boolean;
  onRestore: (entryId: string) => void;
}) => {
  const { t, formatDateTime } = useI18n();
  const [open, setOpen] = useState(entry.current);
  const hasChanges = entry.changes.length > 0;
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
              {/* the entry's label comes from the core */}
              <span className="font-bold text-gray-300">{entry.label}</span>
              {entry.current && (
                <StatusBadge tone="success">
                  {t('mods.modpack.current')}
                </StatusBadge>
              )}
              <CountChips entry={entry} />
            </button>
            <FactLine
              title={factsTitle([
                {
                  label: t('values.key.game'),
                  value: gamePhrase(entry.game_version),
                },
                {
                  label: t('values.key.loader'),
                  value: loaderPhrase(entry.loader, entry.loader_version),
                },
              ])}
            >
              <span className="text-ink-muted">
                {formatDateTime(entry.time)}
              </span>
              <span>{gamePhrase(entry.game_version)}</span>
              {entry.loader !== 'vanilla' && (
                <span>{loaderPhrase(entry.loader, entry.loader_version)}</span>
              )}
            </FactLine>
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
        {open && hasChanges && <ChangeList entry={entry} />}
      </div>
    </div>
  );
};

/** A value that may change: the old and new as a DiffValue, or the one value */
const changed = (from: string, to: string): ReactNode =>
  from === to ? to : <DiffValue from={from} to={to} />;

/** The file counts of an update or restore, as fact rows (only non-zero ones) */
const countRows = (
  t: ReturnType<typeof useI18n>['t'],
  counts: [ChangeKind, number][]
): FactRow[] =>
  counts
    .filter(([, count]) => count > 0)
    .map(([kind, count]) => ({
      label: t(CHANGE_MARKS[kind][2]),
      value: <Num value={count} />,
    }));

const PlanBody = ({
  plan,
  origin,
  entry,
  restoring,
  stopped,
}: {
  plan: ModpackUpdatePlan;
  origin: ModpackOrigin;
  /** the history entry a restore goes back to */
  entry: HistoryEntryInfo | null;
  restoring: boolean;
  stopped: boolean;
}) => {
  const { t, formatDateTime } = useI18n();
  const target = plan.target;
  const counts = countRows(t, [
    ['added', plan.added.length],
    ['replaced', plan.replaced.length],
    ['removed', plan.removed.length],
  ]);
  const fromLoader = loaderPhrase(
    plan.current_loader,
    plan.current_loader_version
  );
  const toLoader = loaderPhrase(target.loader, target.loader_version);
  const rows: FactRow[] = restoring
    ? counts
    : [
        {
          label: target.name,
          value:
            origin.version_number && target.version_number ? (
              <DiffValue
                from={
                  <VersionTag version={origin.version_number} tone="from" />
                }
                to={<VersionTag version={target.version_number} tone="to" />}
              />
            ) : target.version_number ? (
              <VersionTag version={target.version_number} tone="to" />
            ) : null,
        },
        {
          label: t('values.key.game'),
          value: changed(
            gamePhrase(plan.current_game_version),
            gamePhrase(target.game_version)
          ),
        },
        { label: t('values.key.loader'), value: changed(fromLoader, toLoader) },
        ...counts,
      ];
  return (
    <span className="flex flex-col gap-3 text-medium text-white/75">
      {restoring ? (
        <>
          <ObjectCard
            compact
            icon={<FontAwesomeIcon icon={faClock} className="w-4" />}
            // the entry's label comes from the core
            name={entry?.label ?? target.name}
            facts={
              entry && (
                <FactLine>
                  <span className="text-ink-muted">
                    {formatDateTime(entry.time)}
                  </span>
                  <Fact label={t('mods.modpack.files')} icon={faFile}>
                    <Num value={entry.file_count} />
                  </Fact>
                </FactLine>
              )
            }
          />
          <span>{t('mods.modpack.dialog.restoreBody')}</span>
        </>
      ) : (
        <ObjectCard
          compact
          icon={<PackIcon url={origin.icon_url} size="size-6" />}
          name={origin.name}
        />
      )}
      {rows.length > 0 ? (
        <FactList rows={rows} framed />
      ) : (
        <span>{t('mods.modpack.dialog.nothing')}</span>
      )}
      {target.unsupported_reason && (
        <span className="flex flex-col text-red-200">
          {t('mods.modpack.unsupported')}
          {/* the reason comes from the core */}
          <span className="text-small text-red-200/75">
            {target.unsupported_reason}
          </span>
        </span>
      )}
      {plan.reinstalls_server && (
        <span>{t('mods.modpack.dialog.reinstall')}</span>
      )}
      <NameList
        groups={[
          {
            heading: t('mods.modpack.group.removed'),
            items: plan.removed.map((path) => ({
              id: path,
              name: <MonoText text={path} max={44} className="text-small" />,
            })),
          },
          {
            heading: t('mods.modpack.group.kept'),
            items: plan.kept.map((path) => ({
              id: path,
              name: <MonoText text={path} max={44} className="text-small" />,
            })),
          },
        ]}
      />
      {/* the note comes from the core */}
      {plan.note && <span className="text-white/50">{plan.note}</span>}
      {!stopped && (
        <span className="text-yellow">{t('mods.modpack.dialog.stop')}</span>
      )}
    </span>
  );
};

/** The file counts of a finished update, as icon facts in its toast */
const ToastCounts = ({ counts }: { counts: [ChangeKind, number][] }) => {
  const { t, formatNumber } = useI18n();
  const shown = counts.filter(([, count]) => count > 0);
  if (shown.length === 0) return null;
  return (
    <FactLine className="h-auto">
      {shown.map(([kind, count]) => {
        const [icon, , key] = CHANGE_MARKS[kind];
        return (
          <Fact
            key={kind}
            label={t(key)}
            icon={icon}
            title={`${t(key)}: ${formatNumber(count)}`}
          >
            <Num value={count} />
          </Fact>
        );
      })}
    </FactLine>
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
  const { t } = useI18n();
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
      toast.error(
        <ObjectToast
          object={<ObjectChip kind="file" name={file.name} mono />}
          outcome={translate('mods.modpack.notMrpack')}
        />
      );
      return;
    }
    setBusy(true);
    try {
      const info = await uploadModpack(file);
      if (info.unsupported_reason) {
        toast.error(
          <ObjectToast
            object={<ObjectChip kind="modpack" name={info.name} />}
            outcome={translate('mods.modpack.unsupported')}
            reason={info.unsupported_reason}
          />
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
    try {
      const done = await updateModpack(uuid, pending.source);
      toast.success(
        <ObjectToast
          object={
            <>
              <ObjectChip kind="modpack" name={target.name} />
              {!restore && target.version_number && (
                <VersionTag version={target.version_number} tone="to" />
              )}
            </>
          }
          outcome={
            restore
              ? translate('mods.modpack.toast.restored')
              : translate('mods.modpack.toast.updated')
          }
          // what the core skipped, in its words
          reason={done.skipped}
        >
          <ToastCounts
            counts={[
              ['added', done.added],
              ['replaced', done.replaced],
              ['removed', done.removed],
            ]}
          />
        </ObjectToast>
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
  const restoreEntry =
    pending?.source.type === 'history'
      ? (history?.entries.find(
          (e) =>
            pending.source.type === 'history' &&
            e.id === pending.source.entry_id
        ) ?? null)
      : null;

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
            origin={origin}
            entry={restoreEntry}
            restoring={restoring}
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
        <div className="flex min-w-0 flex-col gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <PackIcon url={origin.icon_url} size="size-6" />
            <div className="truncate text-h3 font-bold text-gray-300">
              {origin.name}
            </div>
          </div>
          {(origin.version_number || update) && (
            <FactStrip
              items={[
                ...(origin.version_number
                  ? [
                      {
                        label: t('values.key.installed'),
                        value: <VersionTag version={origin.version_number} />,
                      },
                    ]
                  : []),
                ...(update
                  ? [
                      {
                        label: t('values.key.available'),
                        value: (
                          <>
                            <VersionTag
                              version={update.version_number}
                              tone="to"
                            />
                            <StatusBadge tone="success">
                              {t('mods.modpack.updateAvailable')}
                            </StatusBadge>
                          </>
                        ),
                      },
                    ]
                  : []),
              ]}
            />
          )}
          {update ? (
            update.changelog && (
              <WhatsNewToggle
                open={updateChangelogOpen}
                onToggle={() => setUpdateChangelogOpen(!updateChangelogOpen)}
              />
            )
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
                label={t('mods.modpack.update')}
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
            label={t('mods.modpack.history')}
            badge={
              history && history.entries.length > 0 ? (
                <CountBadge count={history.entries.length} />
              ) : undefined
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
                  entry={entry}
                  last={index === history.entries.length - 1}
                  canWrite={canWrite}
                  busy={busy}
                  onRestore={(entryId) =>
                    plan({ type: 'history', entry_id: entryId })
                  }
                />
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}
