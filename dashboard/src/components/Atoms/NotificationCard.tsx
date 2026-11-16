import ProgressBar from 'components/Atoms/ProgressBar';
import { formatNotificationTime } from 'utils/util';
import LoadingStatusIcon from './LoadingStatusIcon';
import { EventLevel } from 'bindings/EventLevel';
import { NotificationContent, OngoingState } from 'data/NotificationContext';
import { useT } from 'i18n';
import { ReactNode } from 'react';
import { ObjectChip, StatusBadge, stateTone, VersionTag } from './Values';

const NotificationLevelToBgColorClass = (
  level: EventLevel,
  state?: OngoingState
) => {
  switch (level) {
    case 'Warning':
      return 'bg-yellow';
    case 'Error':
      return 'bg-red';
    default:
      switch (state) {
        case 'ongoing':
          return 'bg-gray-500';
        case 'done':
          return 'bg-green';
        case 'error':
          return 'bg-red';
        default:
          return 'bg-gray-500';
      }
  }
};

const outcomeClass = 'text-small font-medium tracking-medium text-white/50';

/** A group of players who joined or left, as chips after its key */
const Players = ({ label, names }: { label: string; names: string[] }) =>
  names.length === 0 ? null : (
    <div className="flex flex-row flex-wrap items-center gap-x-2 text-small">
      <span className="text-white/50">{label}</span>
      {names.map((name, i) => (
        <ObjectChip key={`${i}:${name}`} kind="player" name={name} max="9rem" />
      ))}
    </div>
  );

/**
 * A notification's content, led by its object: the server (or key) as a
 * chip with its state, then the outcome, then the players, message or
 * version. Names are never inside the sentence.
 */
const ContentView = ({ content }: { content: NotificationContent }) => {
  const t = useT();
  const server = (name: string, status?: ReactNode) => (
    <div className="flex min-w-0 flex-row items-center gap-2 text-medium">
      <ObjectChip kind="server" name={name} max="13rem" />
      {status}
    </div>
  );
  switch (content.type) {
    case 'state':
      return (
        <>
          {server(
            content.server,
            <StatusBadge tone={stateTone(content.state)}>
              {t(`common.state.${content.state}`)}
            </StatusBadge>
          )}
          <span className={outcomeClass}>
            {t(`shell.event.outcome.${content.state}`)}
          </span>
        </>
      );
    case 'warning':
    case 'error':
      return (
        <>
          {server(content.server)}
          <span className={outcomeClass}>
            {t(`shell.event.outcome.${content.type}`)}
          </span>
        </>
      );
    case 'players':
      return (
        <>
          {server(content.server)}
          <Players
            label={t('shell.event.outcome.joined')}
            names={content.joined}
          />
          <Players label={t('shell.event.outcome.left')} names={content.left} />
        </>
      );
    case 'chat':
      return (
        <>
          {server(content.server)}
          <div className="flex min-w-0 flex-row flex-wrap items-baseline gap-x-2 text-medium">
            <ObjectChip kind="player" name={content.player} max="9rem" />
            <span className="sr-only">{t('shell.event.outcome.said')}</span>
            {/* <q> quotes the message the way the page's language does */}
            <q className="min-w-0 break-all text-gray-300">{content.message}</q>
          </div>
        </>
      );
    case 'modpack':
      return (
        <>
          <div className="flex min-w-0 flex-row items-center gap-2 text-medium">
            <ObjectChip kind="modpack" name={content.pack} max="11rem" />
            <VersionTag version={content.version} tone="to" />
          </div>
          <span className={outcomeClass}>
            {t('shell.event.outcome.modpackOut')}
          </span>
          <div className="flex flex-row items-center gap-1.5 text-small">
            <span className="text-white/50">{t('values.kind.server')}</span>
            <ObjectChip kind="server" name={content.server} max="12rem" />
          </div>
        </>
      );
    case 'apiKey':
      return (
        <>
          <div className="text-medium">
            <ObjectChip kind="apiKey" name={content.name} max="15rem" />
          </div>
          <span className={outcomeClass}>
            {content.created
              ? t('shell.event.outcome.apiKeyCreated')
              : t('shell.event.outcome.apiKeyRevoked')}
          </span>
        </>
      );
    case 'text':
      // a finished progression: the core's title
      return (
        <p className="w-85 text-medium font-bold tracking-medium break-all">
          {content.title}
        </p>
      );
  }
};

export default function NotificationCard({
  level,
  state,
  content,
  title = '',
  message = '',
  progress_percent = 0,
  timestamp,
}: {
  level: EventLevel;
  state?: OngoingState;
  /** what a finished notification is about; ongoing ones have a title */
  content?: NotificationContent;
  title?: string;
  message?: string;
  progress_percent?: number; // progress in percentage
  timestamp: number;
}) {
  progress_percent = Math.min(0.99, progress_percent); // Prevents the progress bar from being full when the instance is still loading

  return (
    <div className="overflow-hidden rounded-md bg-gray-900">
      <div
        className={`flex flex-row items-center justify-start gap-3 px-4 pt-3 pb-2.5 text-gray-300 hover:bg-gray-900`}
      >
        <LoadingStatusIcon level={level} state={state} />
        <div className="flex w-85 min-w-0 flex-col items-start gap-0.5">
          {content ? (
            <ContentView content={content} />
          ) : (
            <p className="w-85 text-medium font-bold tracking-medium break-all">
              {title}
            </p>
          )}
          <p className="w-85 text-medium font-medium tracking-medium break-all">
            {message}
          </p>
          {!message && (
            <span className="text-small font-medium tracking-medium whitespace-nowrap text-ink-muted">
              {formatNotificationTime(timestamp)}
            </span>
          )}
        </div>
      </div>
      {state && state !== 'done' ? (
        <ProgressBar
          progress_percent={progress_percent}
          colorClass={NotificationLevelToBgColorClass(level, state)}
        />
      ) : (
        <div className="h-1"></div>
      )}
    </div>
  );
}
