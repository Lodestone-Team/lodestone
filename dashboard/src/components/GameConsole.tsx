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
import { useContext, useEffect, useLayoutEffect, useMemo } from 'react';
import { useRef, useState } from 'react';
import { DISABLE_AUTOFILL, errorToString } from 'utils/util';
import ErrorGraphic from './ErrorGraphic';
import { useDocumentTitle } from 'usehooks-ts';
import Checkbox from './Atoms/Checkbox';
import { SkeletonCode, SkeletonGroup } from './Atoms/Skeleton';
import { toast } from 'react-toastify';
import { ConsoleLine } from './ConsoleLine';
import LogLoading from './LogLoading';
import { ParsedConsoleLine, parseConsoleLine } from 'utils/consoleFormat';
import { useColoredConsole } from 'utils/hooks';
import { useResolvedTheme } from 'utils/theme';
import { useLinkedPlayers } from 'data/MinecraftLinks';
import { linkedPlayerInLine } from 'utils/linkedPlayers';
import { MessageKey, useI18n } from 'i18n';
import { compareSnowflakes, MAX_LOADED_LINES } from 'utils/consoleLog';

const autoScrollThreshold = 10;
// closer than this to the top of the list, older lines are shown or fetched
const loadOlderThreshold = 40;
// rendering is the slow part, so only the newest lines are put in the DOM
const maxRenderedLines = 2000;
// more lines put in the DOM when the user scrolls up to the hidden ones
const renderedLinesStep = 1000;

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

/** The index of the line with `snowflake` in `lines` (ordered), or -1. */
const indexOfLine = (lines: ConsoleEvent[], snowflake: string) => {
  let low = 0;
  let high = lines.length - 1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    const order = compareSnowflakes(lines[middle].snowflake, snowflake);
    if (order === 0) return middle;
    if (order < 0) low = middle + 1;
    else high = middle - 1;
  }
  return -1;
};

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
  const {
    consoleLog,
    consoleStatus,
    loadOlder,
    loadingOlder,
    hasOlder,
    historyFailed,
    historyLimitReached,
  } = useConsoleStream(uuid);
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
  const [renderedLines, setRenderedLines] = useState(maxRenderedLines);
  // what the list looked like after the last update, to keep the user's place
  // when lines are added in front of the ones they are reading
  const lastViewRef = useRef({
    uuid,
    firstKey: '',
    scrollHeight: 0,
    filter: '',
    regex: true,
  });
  // the lines in the list's DOM, and the one at the top of the viewport with
  // its distance from the list's top edge: that line stays where it is when
  // lines are added or dropped above it, or the rendered lines slide down
  const renderedRef = useRef<ConsoleEvent[]>([]);
  const anchorRef = useRef<{ snowflake: string; offset: number } | null>(null);

  // the instance has its own lines
  const [renderedLinesUuid, setRenderedLinesUuid] = useState(uuid);
  if (renderedLinesUuid !== uuid) {
    setRenderedLinesUuid(uuid);
    setRenderedLines(maxRenderedLines);
  }

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
      lines: filtered.slice(-renderedLines),
      hiddenCount: Math.max(0, filtered.length - renderedLines),
    };
  }, [consoleLog, filter, regex, renderedLines]);

  // Older lines: first the ones already loaded but not rendered, then the
  // next page from the core.
  const showOlder = () => {
    const list = listRef.current;
    if (list) lastViewRef.current.scrollHeight = list.scrollHeight;
    if (visibleLines.hiddenCount > 0) {
      setRenderedLines((count) => count + renderedLinesStep);
    } else if (hasOlder && !loadingOlder) {
      loadOlder();
    }
  };

  // the first line element whose bottom is below the list's top edge
  const updateAnchor = (list: HTMLOListElement) => {
    const lines = renderedRef.current;
    // the elements before the lines are notices
    const lead = list.children.length - lines.length;
    if (lines.length === 0 || lead < 0) {
      anchorRef.current = null;
      return;
    }
    const top = list.getBoundingClientRect().top;
    let low = 0;
    let high = lines.length - 1;
    while (low < high) {
      const middle = (low + high) >> 1;
      if (list.children[lead + middle].getBoundingClientRect().bottom > top)
        high = middle;
      else low = middle + 1;
    }
    anchorRef.current = {
      snowflake: lines[low].snowflake,
      offset: list.children[lead + low].getBoundingClientRect().top - top,
    };
  };

  const onListScroll = () => {
    const list = listRef.current;
    if (!list) return;
    updateAnchor(list);
    stickToBottomRef.current =
      list.scrollHeight - list.scrollTop - list.clientHeight <
      autoScrollThreshold;
    if (list.scrollTop < loadOlderThreshold) showOlder();
  };

  // if the user was at the bottom of the list, keep them there when new
  // lines are added; if they scrolled up, leave them where they are, also
  // when lines are added above (or dropped from) what they are reading
  useLayoutEffect(() => {
    const list = listRef.current;
    renderedRef.current = visibleLines.lines;
    if (!list) return;
    const last = lastViewRef.current;
    const firstKey = visibleLines.lines[0]?.snowflake ?? '';
    if (last.uuid !== uuid) {
      stickToBottomRef.current = true;
      anchorRef.current = null;
    }
    const anchor = anchorRef.current;
    const anchorIndex = anchor
      ? indexOfLine(visibleLines.lines, anchor.snowflake)
      : -1;
    const lead = list.children.length - visibleLines.lines.length;
    if (stickToBottomRef.current) {
      list.scrollTop = list.scrollHeight;
    } else if (anchor && anchorIndex >= 0 && lead >= 0) {
      const element = list.children[lead + anchorIndex];
      const offset =
        element.getBoundingClientRect().top - list.getBoundingClientRect().top;
      list.scrollTop += offset - anchor.offset;
    } else if (
      firstKey !== last.firstKey &&
      filter === last.filter &&
      regex === last.regex
    ) {
      list.scrollTop += list.scrollHeight - last.scrollHeight;
    }
    lastViewRef.current = {
      uuid,
      firstKey,
      scrollHeight: list.scrollHeight,
      filter,
      regex,
    };
  }, [visibleLines, filter, regex, uuid]);

  // A list too short to scroll cannot ask for older lines by scrolling, and
  // one still at the top after a page (e.g. one without console output) would
  // wait for the next scroll.
  useEffect(() => {
    const list = listRef.current;
    if (!list || loadingOlder || historyFailed) return;
    if (
      list.scrollHeight <= list.clientHeight ||
      (!stickToBottomRef.current && list.scrollTop < loadOlderThreshold)
    )
      showOlder();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visibleLines, hasOlder, loadingOlder, historyFailed]);

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
            className="font-light flex h-0 grow flex-col overflow-y-auto rounded-t-lg border-b border-gray-faded/30 bg-gray-900 py-3 font-mono text-small tracking-tight break-words whitespace-pre-wrap text-gray-300 [overflow-anchor:none]"
            ref={listRef}
            onScroll={onListScroll}
          >
            {historyLimitReached && visibleLines.hiddenCount === 0 && (
              <li className="px-4 py-0.5 text-gray-500 italic">
                {t('console.historyLimit', {
                  max: formatNumber(MAX_LOADED_LINES),
                })}
              </li>
            )}
            {visibleLines.hiddenCount > 0 && (
              <li className="px-4 py-0.5 text-gray-500 italic">
                {t('console.hiddenLines', {
                  count: visibleLines.hiddenCount,
                  max: formatNumber(renderedLines),
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
        {loadingOlder && (
          <div className="pointer-events-none absolute top-1 left-3 z-10 rounded-md bg-gray-850 text-gray-300 shadow-md">
            <LogLoading loadingText={t('console.loadingOlder')} />
          </div>
        )}
        {historyFailed && (
          <div className="pointer-events-none absolute top-1 left-3 z-10 rounded-md bg-gray-850 px-2 py-1 text-small text-red-200 italic shadow-md">
            {t('console.historyFailed')}
          </div>
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
