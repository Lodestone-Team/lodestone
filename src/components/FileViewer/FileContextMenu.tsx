import React, { useState, useEffect, forwardRef } from 'react';
import ContextMenuButton from 'components/Atoms/ContextMenuButton';
import { toast } from 'react-toastify';
import { ClientFile } from 'bindings/ClientFile';
import { unzipInstanceFile } from 'utils/apis';

const FileContextMenu = forwardRef(
  (
    {
      file,
      currentPath,
      coords,
      setCreateFileModalOpen,
      setCreateFolderModalOpen,
      setClipboard,
      setClipboardAction,
      setTickedFiles,
      pasteFiles,
      setModalPath,
      tickedFiles,
      clipboard,
      setShowContextMenu,
      unzipFile,
      setRenameFileModalOpen,
      deleteSingleFile,
      deleteTickedFiles,
    }: {
      file: ClientFile | null;
      coords: { x: number; y: number };
      currentPath: string;
      setCreateFileModalOpen: (modalOpen: boolean) => void;
      setCreateFolderModalOpen: (modalOpen: boolean) => void;
      setRenameFileModalOpen: (modalOpen: boolean) => void;
      setShowContextMenu: (showContextMenu: boolean) => void;
      setClipboard: (clipboard: ClientFile[]) => void;
      unzipFile: (file: ClientFile) => void;
      pasteFiles: (path: string) => void;
      setClipboardAction: (clipboardAction: 'copy' | 'cut') => void;
      setTickedFiles: (tickedFiles: ClientFile[]) => void;
      setModalPath: (modalPath: string) => void;
      deleteSingleFile: (file: ClientFile) => void;
      deleteTickedFiles: () => void;
      tickedFiles: ClientFile[] | undefined;
      clipboard: ClientFile[] | undefined;
    },
    ref: React.Ref<HTMLDivElement>
  ) => {
    const [isMac, setIsMac] = useState(false);
    useEffect(() => {
      if (window.navigator.userAgent.indexOf('Mac') != -1) {
        setIsMac(true);
      }
    }, []);

    const deleteFile = async () => {
      if (tickedFiles?.includes(file as ClientFile)) {
        await deleteTickedFiles();
      } else {
        await deleteSingleFile(file as ClientFile);
      }
      setTickedFiles([]);
      toast.info('Files deleted');
    };

    const cutFile = async () => {
      if (tickedFiles?.includes(file as ClientFile)) {
        setClipboard(tickedFiles);
      } else {
        setClipboard([file as ClientFile]);
      }
      setTickedFiles([]);
      setClipboardAction('cut');
      toast.info('Files cut to clipboard');
    };

    return (
      <div
        className="fixed right-0 z-50 mt-1.5 w-44 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-900 drop-shadow-md focus:outline-none"
        style={{
          top: coords.y + 'px',
          left: coords.x + 'px',
          position: 'absolute',
        }}
        ref={ref}
      >
        <div className="py-2">
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Copy"
            // subLabel={ isMac ? "⌘+C" : "CTRL+C"}
            disabled={true}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Cut"
            // subLabel={ isMac ? "⌘+X" : "CTRL+X"}
            onClick={() => {
              cutFile();
              setShowContextMenu(false);
            }}
            disabled={file === null}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label={file?.file_type !== 'Directory' ? "Paste" : `Paste into ${file?.name}`}
            // subLabel={ isMac ? "⌘+C" : "CTRL+C"}
            onClick={() => {
              pasteFiles(file?.file_type !== 'Directory' ? currentPath : file?.path);
              setShowContextMenu(false);
            }}
            disabled={clipboard?.length === 0}
          />
        </div>
        <div className="py-2">
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Rename"
            // subLabel={ isMac ? "⌘+R" : "CTRL+R"}
            onClick={() => {
              setModalPath((file as ClientFile).path);
              setRenameFileModalOpen(true);
              setShowContextMenu(false);
            }}
            disabled={file === null}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Delete"
            // iconComponent={<BackspaceIcon className="h-3.5 w-3.5 text-gray-300 opacity-50 group-hover:opacity-100" />}
            onClick={() => {
              deleteFile();
              setShowContextMenu(false);
            }}
            disabled={file === null}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Unzip"
            onClick={() => {
              unzipFile(file as ClientFile);
              setShowContextMenu(false);
            }}
            disabled={file === null}
          />
        </div>
        <div className="py-2">
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="New folder"
            onClick={() => {
              setCreateFolderModalOpen(true);
              setShowContextMenu(false);
            }}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="New file"
            onClick={() => {
              setCreateFileModalOpen(true);
              setShowContextMenu(false);
            }}
          />
        </div>
      </div>
    );
  }
);

FileContextMenu.displayName = 'FileContextMenu';

export default FileContextMenu;