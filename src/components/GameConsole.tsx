import axios from 'axios';
import { useConsoleStream } from 'data/ConsoleStream';
import { LodestoneContext } from 'data/LodestoneContext';
import { useContext, useRef, useState } from 'react';

export default function GameConsole({
  uuid,
  enableInput = true,
}: {
  uuid: string;
  enableInput?: boolean;
}) {
  const lodestoneContex = useContext(LodestoneContext);
  const {consoleLog, consoleStatus} = useConsoleStream(uuid);
  const [command, setCommand] = useState('');
  const listRef = useRef<HTMLOListElement>(null);

  enableInput = enableInput && consoleStatus === 'live';

  const sendCommand = (command: string) => {
    axios.post(`/instance/${uuid}/console?command=${command}`).then(() => {
      // scroll to bottom
      listRef.current?.scrollTo(0, listRef.current.scrollHeight);
    });
  };

  let consoleStatusMessage = '';
  switch (consoleStatus) {
    case 'no-permission':
      consoleStatusMessage = 'You do not have permission to access this console.';
      break;
    case 'loading':
      consoleStatusMessage = 'Loading console...';
      break;
    case 'buffered':
      consoleStatusMessage = 'Console is buffered. Some messages may be missing.';
      break;
    case 'live':
      consoleStatusMessage = 'Console is live.';
      break;
    case 'closed':
      consoleStatusMessage = 'Console is closed.';
      break;
    case 'error':
      consoleStatusMessage = 'Connection lost or error.';
  }

  return (
    <div className="flex flex-col w-full">
      <ol
        className="flex flex-col overflow-y-auto overflow-x-auto whitespace-pre-wrap break-words rounded-t-lg font-mono text-small font-light tracking-tight text-gray-300 bg-[#101010] pt-3 pb-32 h-[40vh] border-gray-faded/30 border-b-2"
        ref={listRef}
      >
        {consoleLog.map((line) => (
          <li key={line.idempotency} className="hover:bg-gray-800 py-[0.125rem] px-4">
            {line.event_inner.InstanceOutput}
          </li>
        ))}
        {consoleStatusMessage && (
          <li className="text-gray-500 py-[0.125rem] px-4">▲ {consoleStatusMessage}</li>
        )}
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
            className="w-full bg-[#101010] placeholder:text-gray-500 text-gray-300 p-3 outline-gray-300 focus-visible:outline-2 focus-visible:outline rounded-b-lg disabled:placeholder:text-gray-600"
            placeholder={
              enableInput ? 'Enter command...' : 'Instance is not running'
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
