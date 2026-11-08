import { faCircle, faServer } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import axios from 'axios';
import { ConsoleEvent, useConsoleStream } from 'data/ConsoleStream';
import { InstanceContext } from 'data/InstanceContext';
import { CommandHistoryContext } from 'data/CommandHistoryContext';
import { useUserAuthorized } from 'data/UserInfo';
import {
  OWNER_ONLY_ATOM_MESSAGE,
  useCanControlInstance,
} from 'data/CodeExecution';
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

const autoScrollThreshold = 10;
// rendering is the slow part, so only the newest lines are put in the DOM
const maxRenderedLines = 2000;

// each line is parsed once; the log keeps the same event objects as it grows
const parsedLines = new WeakMap<ConsoleEvent, ParsedConsoleLine>();
const parsed = (line: ConsoleEvent) => {
  let result = parsedLines.get(line);
  if (!result) {
    result = parseConsoleLine(line.message);
    parsedLines.set(line, result);
  }
  return result;
};

export default function GameConsole() {
  useDocumentTitle('Instance Console - Lodestone');
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
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
      toast.error(`Failed to send command: ${errorToString(e)}`);
    }
  };

  let consoleStatusMessage = '';
  let consoleStatusColor = 'text-gray-500';
  switch (consoleStatus) {
    case 'no-permission':
      consoleStatusMessage = 'No permission to access console';
      consoleStatusColor = 'text-gray-500';
      break;
    case 'loading':
      consoleStatusMessage = 'Loading console...';
      consoleStatusColor = 'text-gray-500';
      break;
    case 'buffered':
      consoleStatusMessage = 'History messages. No live updates';
      consoleStatusColor = 'text-gray-500';
      break;
    case 'live':
      consoleStatusMessage = 'Console is live';
      consoleStatusColor = 'text-green-200';
      break;
    case 'live-no-buffer':
      consoleStatusMessage =
        'Console is live but failed to fetch history. Your internet connection may be unstable';
      consoleStatusColor = 'text-yellow';
      break;
    case 'closed':
      consoleStatusMessage = 'Console is closed';
      consoleStatusColor = 'text-red-200';
      break;
    case 'error':
      consoleStatusMessage = 'Connection lost or error';
      consoleStatusColor = 'text-red-200';
  }
  // overwrites
  if (instance.state === 'Stopped') {
    consoleStatusMessage = `Instance is ${instance.state.toLowerCase()}`;
    consoleStatusColor = 'text-gray-500';
  }

  let consoleInputMessage = '';
  if (!canAccessConsole || consoleStatus === 'no-permission')
    consoleInputMessage = 'No permission';
  else if (!canControlInstance) consoleInputMessage = OWNER_ONLY_ATOM_MESSAGE;
  else if (instance.state === 'Stopped')
    consoleInputMessage = `Instance is ${instance.state.toLowerCase()}`;
  else if (consoleStatus === 'closed')
    consoleInputMessage = 'Console is closed';

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
  setRegex(checked)
  }

  return (
    <div className="relative flex h-full w-full grow flex-col pt-0">
      <div className="font-mono text-small mb-3 flex flex-row w-full justify-between">
            <input
              className="w-full rounded-lg bg-gray-850 py-1 px-4 text-gray-300 placeholder:text-gray-500 focus:outline-hidden disabled:placeholder:text-gray-500"
              placeholder={'Search console...'}
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              id="search"
              type="text"
            />
          <Checkbox label='Regex' checked={regex} onChange={checkRegex} className="ml-3"/>
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
            className={`absolute top-1 right-1 select-none p-1.5 text-small ${consoleStatusColor}`}
          />
        </Tooltip>
        {!canAccessConsole || consoleStatus === 'no-permission' ? (
          <ErrorGraphic
            icon={faServer}
            message="You don't have permission to access this console"
            className="border-b border-gray-faded/30"
            iconClassName="text-gray-400"
            messageClassName="text-white/50"
          />
        ) : consoleLog.length === 0 && consoleStatus === 'loading' ? (
          // the history is still loading: an empty console, not the "no
          // messages" graphic that would flash before the lines appear
          <div
            aria-busy="true"
            className="flex h-0 grow rounded-t-lg border-b border-gray-faded/30 bg-gray-900"
          />
        ) : consoleLog.length === 0 && consoleStatus === 'loading' ? (
          // the history's lines, while it loads (after a moment)
          <SkeletonGroup
            label="Loading console"
            className="flex h-0 grow overflow-hidden rounded-t-lg border-b border-gray-faded/30 bg-gray-900 px-4 py-3"
          >
            <SkeletonCode lines={18} className="w-full" />
          </SkeletonGroup>
        ) : consoleLog.length === 0 ? (
          <ErrorGraphic
            icon={faServer}
            message="No console messages yet"
            className="border-b border-gray-faded/30"
            iconClassName="text-gray-400"
            messageClassName="text-white/50"
          />
        ) : (
          <ol
            className="font-light flex h-0 grow rounded-t-lg flex-col overflow-y-auto whitespace-pre-wrap break-words border-b border-gray-faded/30 bg-gray-900 py-3 font-mono text-small tracking-tight text-gray-300"
            ref={listRef}
            onScroll={onListScroll}
          >
            {visibleLines.hiddenCount > 0 && (
              <li className="px-4 py-0.5 text-gray-500 italic">
                {visibleLines.hiddenCount.toLocaleString()} older lines hidden
                (showing the newest {maxRenderedLines.toLocaleString()})
              </li>
            )}
            {visibleLines.lines.map((line) => (
              <ConsoleLine
                key={line.snowflake}
                line={parsed(line)}
                colored={coloredConsole}
                theme={theme}
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
              className="w-full rounded-b-lg bg-gray-850 py-3 px-4 text-gray-300 focus:outline-hidden placeholder:text-gray-500 disabled:placeholder:text-gray-500"
              placeholder={consoleInputMessage || 'Enter command...'}
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
