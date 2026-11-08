import { faCircle, faServer } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import axios from 'axios';
import {
  ConsoleEvent,
  ConsoleStreamStatus,
  useConsoleStream,
} from 'data/ConsoleStream';
import { InstanceContext } from 'data/InstanceContext';
import { CommandHistoryContext } from 'data/CommandHistoryContext';
import { useUserAuthorized } from 'data/UserInfo';
import { useCanControlInstance } from 'data/CodeExecution';
import Tooltip from 'rc-tooltip';
import { useContext, useLayoutEffect, useMemo } from 'react';
import { useRef, useState } from 'react';
import { DISABLE_AUTOFILL, errorToString } from 'utils/util';
import ErrorGraphic from './ErrorGraphic';
import { useDocumentTitle } from 'usehooks-ts';
import Checkbox from './Atoms/Checkbox';
import { SkeletonCode, SkeletonGroup } from './Atoms/Skeleton';
import { toast } from 'react-toastify';
import { ConsoleLine } from './ConsoleLine';
import { ParsedConsoleLine, parseConsoleLine } from 'utils/consoleFormat';
import { useColoredConsole } from 'utils/hooks';
import { useResolvedTheme } from 'utils/theme';
import { useLinkedPlayers } from 'data/MinecraftLinks';
import { linkedPlayerInLine } from 'utils/linkedPlayers';
import { MessageKey, useI18n } from 'i18n';

const autoScrollThreshold = 10;
// rendering is the slow part, so only the newest lines are put in the DOM
const maxRenderedLines = 2000;

// each line is parsed once; the log keeps the same event objects as it grows
const parsedLines = new WeakMap<ConsoleEvent, ParsedConsoleLine>();
// the status dot's tooltip for each stream status
const statusMessages = {
  'no-permission': 'console.status.noPermission',
  loading: 'console.status.loading',
  buffered: 'console.status.buffered',
  live: 'console.status.live',
  'live-no-buffer': 'console.status.liveNoBuffer',
  closed: 'console.status.closed',
  error: 'console.status.error',
} as const satisfies Record<ConsoleStreamStatus, MessageKey>;

const parsed = (line: ConsoleEvent) => {
  let result = parsedLines.get(line);
  if (!result) {
    result = parseConsoleLine(line.message);
    parsedLines.set(line, result);
  }
  return result;
};

export default function GameConsole() {
  const { t, formatNumber } = useI18n();
  useDocumentTitle(t('common.pageTitle', { page: t('console.pageTitle') }));
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No server selected');
  const uuid = instance.uuid;
  const canAccessConsole = useUserAuthorized(
    'can_access_instance_console',
    uuid
  );
  // only the owner can send input to atom instances
  const canControlInstance = useCanControlInstance(instance);
  const { consoleLog, consoleStatus } = useConsoleStream(uuid);
  const [command, setCommand] = useState('');
  const [filter, setFilter] = useState('');
  const [regex, setRegex] = useState(true);
  const [coloredConsole] = useColoredConsole();
  const theme = useResolvedTheme();
  const linked = useLinkedPlayers();
  const { commandHistory, appendCommandHistory } = useContext(
    CommandHistoryContext
  );
  const [commandNav, setCommandNav] = useState(commandHistory.length);
  const listRef = useRef<HTMLOListElement>(null);
  // whether the user was at the bottom of the list the last time they
  // scrolled; starts true so the console opens at the newest line
  const stickToBottomRef = useRef(true);

  const onListScroll = () => {
    const list = listRef.current;
    if (!list) return;
    stickToBottomRef.current =
      list.scrollHeight - list.scrollTop - list.clientHeight <
      autoScrollThreshold;
  };

  const scrollToBottom = () => {
    if (listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
    stickToBottomRef.current = true;
  };

  // one compiled filter for the whole list, rebuilt only when it changes
  const visibleLines = useMemo(() => {
    let matches: (line: ConsoleEvent) => boolean = () => true;
    if (filter !== '') {
      if (regex) {
        let filterRegex: RegExp | null = null;
        try {
          filterRegex = new RegExp(filter, 'i');
        } catch {
          // an invalid regex matches nothing, as before
        }
        matches = (line) => filterRegex?.test(parsed(line).plain) ?? false;
      } else {
        const needle = filter.toLowerCase();
        matches = (line) => parsed(line).plain.toLowerCase().includes(needle);
      }
    }
    const filtered = filter === '' ? consoleLog : consoleLog.filter(matches);
    return {
      lines: filtered.slice(-maxRenderedLines),
      hiddenCount: Math.max(0, filtered.length - maxRenderedLines),
    };
  }, [consoleLog, filter, regex]);

  // if the user was at the bottom of the list, keep them there when new
  // lines are added; if they scrolled up, leave them where they are
  useLayoutEffect(() => {
    if (stickToBottomRef.current && listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [visibleLines]);

  const sendCommand = async (command: string) => {
    scrollToBottom();
    try {
      await axios({
        method: 'post',
        url: `/instance/${uuid}/console`,
        data: JSON.stringify(command),
        headers: {
          'Content-Type': 'application/json',
        },
      });
    } catch (e) {
      toast.error(t('console.sendFailed', { error: errorToString(e) }));
    }
  };

  let consoleStatusMessage = t(statusMessages[consoleStatus]);
  let consoleStatusColor = 'text-gray-500';
  switch (consoleStatus) {
    case 'live':
      consoleStatusColor = 'text-green-200';
      break;
    case 'live-no-buffer':
      consoleStatusColor = 'text-yellow';
      break;
    case 'closed':
    case 'error':
      consoleStatusColor = 'text-red-200';
  }
  // overwrites
  if (instance.state === 'Stopped') {
    consoleStatusMessage = t('console.serverStopped');
    consoleStatusColor = 'text-gray-500';
  }

  let consoleInputMessage = '';
  if (!canAccessConsole || consoleStatus === 'no-permission')
    consoleInputMessage = t('console.input.noPermission');
  else if (!canControlInstance)
    consoleInputMessage = t('console.ownerOnlyAtom');
  else if (instance.state === 'Stopped')
    consoleInputMessage = t('console.serverStopped');
  else if (consoleStatus === 'closed')
    consoleInputMessage = t('console.status.closed');

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowUp') {
      setCommandNav((prev) => {
        prev = Math.max(prev - 1, 0);
        setCommand(commandHistory[prev]);
        return prev;
      });
    } else if (event.key === 'ArrowDown') {
      setCommandNav((prev) => {
        prev = Math.min(prev + 1, commandHistory.length - 1);
        setCommand(commandHistory[prev]);
        return prev;
      });
    } else {
      setCommandNav(commandHistory.length);
    }
  };

  const checkRegex = (checked: boolean) => {
    setRegex(checked);
  };

  return (
    <div className="relative flex h-full w-full grow flex-col pt-0">
      <div className="mb-3 flex w-full flex-row justify-between font-mono text-small">
        <input
          className="w-full rounded-lg bg-gray-850 px-4 py-1 text-gray-300 placeholder:text-gray-500 focus:outline-hidden disabled:placeholder:text-gray-500"
          placeholder={t('console.searchPlaceholder')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          id="search"
          type="text"
        />
        <Checkbox
          label={t('console.regex')}
          checked={regex}
          onChange={checkRegex}
          className="ml-3"
        />
      </div>
      <div className="relative flex h-full w-full grow flex-col rounded-lg border border-gray-faded/30">
        <Tooltip
          overlay={<span>{consoleStatusMessage}</span>}
          placement="bottom"
          showArrow={false}
          trigger={['hover']}
          mouseEnterDelay={0}
        >
          <FontAwesomeIcon
            icon={faCircle}
            className={`absolute top-1 right-1 p-1.5 text-small select-none ${consoleStatusColor}`}
          />
        </Tooltip>
        {!canAccessConsole || consoleStatus === 'no-permission' ? (
          <ErrorGraphic
            icon={faServer}
            message={t('console.noPermission')}
            className="border-b border-gray-faded/30"
            iconClassName="text-gray-400"
            messageClassName="text-white/50"
          />
        ) : consoleLog.length === 0 && consoleStatus === 'loading' ? (
          // the history's lines, while it loads (after a moment)
          <SkeletonGroup
            label={t('console.loading')}
            className="flex h-0 grow overflow-hidden rounded-t-lg border-b border-gray-faded/30 bg-gray-900 px-4 py-3"
          >
            <SkeletonCode lines={18} className="w-full" />
          </SkeletonGroup>
        ) : consoleLog.length === 0 ? (
          <ErrorGraphic
            icon={faServer}
            message={t('console.empty')}
            className="border-b border-gray-faded/30"
            iconClassName="text-gray-400"
            messageClassName="text-white/50"
          />
        ) : (
          <ol
            className="font-light flex h-0 grow flex-col overflow-y-auto rounded-t-lg border-b border-gray-faded/30 bg-gray-900 py-3 font-mono text-small tracking-tight break-words whitespace-pre-wrap text-gray-300"
            ref={listRef}
            onScroll={onListScroll}
          >
            {visibleLines.hiddenCount > 0 && (
              <li className="px-4 py-0.5 text-gray-500 italic">
                {t('console.hiddenLines', {
                  count: visibleLines.hiddenCount,
                  max: formatNumber(maxRenderedLines),
                })}
              </li>
            )}
            {visibleLines.lines.map((line) => (
              <ConsoleLine
                key={line.snowflake}
                line={parsed(line)}
                colored={coloredConsole}
                theme={theme}
                linkedUser={
                  linkedPlayerInLine(parsed(line).plain, linked)?.user
                }
              />
            ))}
          </ol>
        )}
        <div className="font-mono text-small">
          <form
            noValidate
            autoComplete={DISABLE_AUTOFILL}
            onSubmit={(e: React.SyntheticEvent) => {
              e.preventDefault();
              sendCommand(command);
              appendCommandHistory(command);
              setCommandNav((prev) => prev + 1);
              setCommand('');
            }}
          >
            <input
              className="w-full rounded-b-lg bg-gray-850 px-4 py-3 text-gray-300 placeholder:text-gray-500 focus:outline-hidden disabled:placeholder:text-gray-500"
              placeholder={
                consoleInputMessage || t('console.input.placeholder')
              }
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              id="command"
              type="text"
              disabled={consoleInputMessage !== ''}
              onKeyDown={handleKeyDown}
            />
          </form>
        </div>
      </div>
    </div>
  );
}
