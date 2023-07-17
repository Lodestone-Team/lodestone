import React from 'react';

interface CommandHistoryContextInterface{
  commandHistory: Array<string>;
  appendCommandHistory: (command: string) => void;
}

export const CommandHistoryContext =
  React.createContext<CommandHistoryContextInterface>({
    commandHistory: [],
    appendCommandHistory: (command) => {
      throw new Error('Not implemented');
    }
  });

export const CommandHistoryContextProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  const [commandLog, setCommandLog] = React.useState<string[]>([]);
  const appendCommandHistory = (command: string) => {
    if (commandLog.length > 50) {
      setCommandLog((prev) => [...prev.slice(1), command]);
    } else {
      setCommandLog((prev) => [...prev, command]);
    }
  }

  const contextValue = {
    commandHistory: commandLog,
    appendCommandHistory
  };

  return (
    <CommandHistoryContext.Provider value={contextValue}>
      {children}
    </CommandHistoryContext.Provider>
  );
};