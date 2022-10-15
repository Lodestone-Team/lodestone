import axios from 'axios';
import { useConsoleStream } from 'data/ConsoleStream';
import { isUserAuthorized, useUserInfo } from 'data/UserInfo';
import { useEffect } from 'react';
import { useRef, useState } from 'react';
import { usePrevious } from 'utils/hooks';

const autoScrollThreshold = 100;

export default function GameConsole({
  uuid,
  enableInput = true,
}: {
  uuid: string;
  enableInput?: boolean;
}) {
  const { consoleLog, consoleStatus } = useConsoleStream(uuid);
  const [command, setCommand] = useState('');
  const { data: userInfo } = useUserInfo();
  const canAccessConsole = isUserAuthorized(userInfo, 'CanAccessConsole', uuid);
  const listRef = useRef<HTMLOListElement>(null);
  const isAtBottom = listRef.current
    ? listRef.current.scrollHeight -
        listRef.current.scrollTop -
        listRef.current.clientHeight <
      autoScrollThreshold
    : true;
  const oldIsAtBottom = usePrevious(isAtBottom);

  enableInput = enableInput && canAccessConsole;

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
  switch (consoleStatus) {
    case 'no-permission':
      consoleStatusMessage =
        'You do not have permission to access this console';
      break;
    case 'loading':
      consoleStatusMessage = 'Loading console...';
      break;
    case 'buffered':
      consoleStatusMessage = 'History messages. No live updates';
      break;
    case 'live':
      consoleStatusMessage = 'Console is live';
      break;
    case 'live-no-buffer':
      consoleStatusMessage =
        'Console is live but failed to fetch history. Your internet connection may be unstable';
      break;
    case 'closed':
      consoleStatusMessage = 'Console is closed';
      break;
    case 'error':
      consoleStatusMessage = 'Connection lost or error';
  }

  return (
    <div className="relative flex flex-col w-full h-full border border-gray-faded/30 rounded-2xl">
      {consoleStatusMessage && (
        <div className="absolute top-0 right-0 p-4 py-1 font-mono font-light tracking-tight text-gray-500 select-none hover:text-gray-400 text-small">
          {consoleStatusMessage}
        </div>
      )}
      <ol
        className="flex flex-col overflow-y-auto overflow-x-auto whitespace-pre-wrap break-words rounded-t-2xl font-mono text-small font-light tracking-tight text-gray-300 bg-[#101010] py-3 h-0 grow border-gray-faded/30 border-b"
        ref={listRef}
      >
        {consoleLog.map((line) => (
          <li
            key={line.idempotency}
            className="hover:bg-gray-800 py-[0.125rem] px-4"
          >
            {line.message}
          </li>
        ))}
      </ol>
      <div className="font-mono text-small">
        <form
          noValidate
          autoComplete="off"
          onSubmit={(e: React.SyntheticEvent) => {
            e.preventDefault();
            sendCommand(command);
            setCommand('');
          }}
        >
          <input
            className="w-full bg-[#101010] placeholder:text-gray-500 text-gray-300 py-3 px-4 outline-gray-300 focus-visible:outline-2 outline-white/50 focus-visible:outline rounded-b-2xl disabled:placeholder:text-gray-600"
            placeholder={
              enableInput ? 'Enter command...' : 'Server is not running or insufficient permissions'
            }
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            id="command"
            type="text"
            disabled={!enableInput}
          />
        </form>
      </div>
    </div>
  );
}
