import { faCircle, faServer } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import axios from 'axios';
import { useConsoleStream } from 'data/ConsoleStream';
import { InstanceContext } from 'data/InstanceContext';
import { useUserAuthorized } from 'data/UserInfo';
import Tooltip from 'rc-tooltip';
import { useContext, useEffect } from 'react';
import { useRef, useState } from 'react';
import { usePrevious } from 'utils/hooks';
import { DISABLE_AUTOFILL } from 'utils/util';

const autoScrollThreshold = 100;

export default function GameConsole() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const uuid = instance.uuid;
  const canAccessConsole = useUserAuthorized(
    'can_access_instance_console',
    uuid
  );
  const { consoleLog, consoleStatus } = useConsoleStream(uuid);
  const [command, setCommand] = useState('');
  const listRef = useRef<HTMLOListElement>(null);
  const isAtBottom = listRef.current
    ? listRef.current.scrollHeight -
        listRef.current.scrollTop -
        listRef.current.clientHeight <
      autoScrollThreshold
    : true;
  const oldIsAtBottom = usePrevious(isAtBottom);

  const scrollToBottom = () => {
    if (listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  };

  // if the user is already at the bottom of the list, scroll to the bottom when new items are added
  // otherwise, don't scroll
  useEffect(() => {
    if (oldIsAtBottom) scrollToBottom();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [consoleLog]);

  const sendCommand = (command: string) => {
    axios({
      method: 'post',
      url: `/instance/${uuid}/console`,
      data: JSON.stringify(command),
      headers: {
        'Content-Type': 'application/json',
      },
    });
    scrollToBottom();
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
  if (instance.state !== 'Running') {
    consoleStatusMessage = `Instance is ${instance.state.toLowerCase()}`;
    consoleStatusColor = 'text-gray-500';
  }

  let consoleInputMessage = '';
  if (!canAccessConsole || consoleStatus === 'no-permission')
    consoleInputMessage = 'No permission';
  else if (instance.state !== 'Running')
    consoleInputMessage = `Instance is ${instance.state.toLowerCase()}`;
  else if (consoleStatus === 'closed')
    consoleInputMessage = 'Console is closed';

  return (
    <div className="relative flex h-full w-full flex-col rounded-lg border border-gray-faded/30">
      <Tooltip
        overlay={<span>{consoleStatusMessage}</span>}
        placement="top"
        trigger={['hover']}
        mouseEnterDelay={0}
      >
        <FontAwesomeIcon
          icon={faCircle}
          className={`absolute top-0 right-0 select-none p-1.5 text-small ${consoleStatusColor}`}
        />
      </Tooltip>
      {!canAccessConsole || consoleStatus === 'no-permission' ? (
        <div className="flex h-full w-full flex-col items-center justify-center gap-4 rounded-t-lg border-b border-gray-faded/30 bg-gray-800">
          <FontAwesomeIcon
            icon={faServer}
            className="text-xlarge text-gray-400"
          />
          <p className="text-xl text-center text-gray-300">
            You don&#39;t have permission to access this console
          </p>
        </div>
      ) : consoleLog.length === 0 ? (
        <div className="flex h-full w-full flex-col items-center justify-center gap-4 rounded-t-lg border-b border-gray-faded/30 bg-gray-800">
          <FontAwesomeIcon
            icon={faServer}
            className="text-xlarge text-gray-400"
          />
          <p className="text-xl text-center text-gray-300">
            No console messages
          </p>
        </div>
      ) : (
        <ol
          className="flex h-0 grow flex-col overflow-y-auto whitespace-pre-wrap break-words rounded-t-lg border-b border-gray-faded/30 bg-gray-900 py-3 font-mono text-small font-light tracking-tight text-gray-300"
          ref={listRef}
        >
          {consoleLog.map((line) => (
            <li
              key={line.snowflake_str}
              className="py-[0.125rem] px-4 hover:bg-gray-800"
            >
              {line.message}
            </li>
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
            setCommand('');
          }}
        >
          <input
            className="w-full rounded-b-lg bg-gray-850 py-3 px-4 text-gray-300 outline-white/50 placeholder:text-gray-500 focus-visible:outline focus-visible:outline-2 disabled:placeholder:text-gray-500"
            placeholder={consoleInputMessage || 'Enter command...'}
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            id="command"
            type="text"
            disabled={consoleInputMessage !== ''}
          />
        </form>
      </div>
    </div>
  );
}
