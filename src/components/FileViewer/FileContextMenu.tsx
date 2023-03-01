import React, { useState, useEffect, forwardRef } from 'react';
import ContextMenuButton from 'components/Atoms/ContextMenuButton';
import { toast } from 'react-toastify';
import { ClientFile } from 'bindings/ClientFile';
import { BackspaceIcon } from "@heroicons/react/24/outline";

const FileContextMenu = forwardRef(
  (
    {
      file,
      coords,
      setCreateFileModalOpen,
      setCreateFolderModalOpen,
      setClipboard,
      setClipboardAction,
      setTickedFiles,
      tickedFiles,
      setShowContextMenu,
    } : {
      file: ClientFile,
      coords: {x: number, y: number},
      setCreateFileModalOpen: (modalOpen: boolean) => void,
      setCreateFolderModalOpen: (modalOpen: boolean) => void,
      setShowContextMenu: (showContextMenu: boolean) => void,
      setClipboard: (clipboard: ClientFile[]) => void;
      setClipboardAction: (clipboardAction: 'copy' | 'cut') => void;
      setTickedFiles: (tickedFiles: ClientFile[]) => void;
      tickedFiles: ClientFile[];
    },
    ref: React.Ref<HTMLDivElement>
  ) => {


    const [ isMac, setIsMac ] = useState(false)
    useEffect(() => {
      if (window.navigator.userAgent.indexOf("Mac") != -1) {
        setIsMac(true)
      }
    }, [])

    const cutFile = async () => {
      if (tickedFiles.includes(file)) {
        setClipboard(tickedFiles);
      } else {
        setClipboard([file]);
      }
      setTickedFiles([]);
      setClipboardAction('cut');
      toast.info('Files cut to clipboard');
    }

    return (
      <div className="fixed right-0 z-50 mt-1.5 w-44 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-900 drop-shadow-md focus:outline-none"
        style={{ top: coords.y + "px", left: coords.x + "px", position: "absolute" }}
        ref={ref}
      >
        <div className="py-2">
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Copy"
            subLabel={ isMac ? "⌘+C" : "CTRL+C"}
            align="end"
            variant="text"
            intention="primary"
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Cut"
            align="end"
            subLabel={ isMac ? "⌘+X" : "CTRL+X"}
            variant="text"
            intention="primary"
            onClick={() => {cutFile(); }}
          />
        </div>
        <div className="py-2">
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Rename"
            align="end"
            variant="text"
            subLabel={ isMac ? "⌘+R" : "CTRL+R"}
            intention="primary"
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Delete"
            align="end"
            variant="text"
            iconComponent={<BackspaceIcon className="h-3.5 w-3.5 text-gray-300 opacity-50 group-hover:opacity-100" />}
            intention="primary"
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Unzip"
            align="end"
            variant="text"
            intention="primary"
          />
        </div>
        <div className="py-2">
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="New folder"
            align="end"
            variant="text"
            intention="primary"
            onClick={() => { setCreateFolderModalOpen(true); setShowContextMenu(false); }}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="New file"
            align="end"
            variant="text"
            intention="primary"
            onClick={() => { setCreateFileModalOpen(true); setShowContextMenu(false); }}
          />
        </div>
      </div>
    );
});

FileContextMenu.displayName = 'FileContextMenu';

export default FileContextMenu;
