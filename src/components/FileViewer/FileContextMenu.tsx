import React, { useState, useEffect, forwardRef } from 'react';
import ContextMenuButton from 'components/Atoms/ContextMenuButton';
import { toast } from 'react-toastify';
import { ClientFile } from 'bindings/ClientFile';
import { unzipInstanceFile } from 'utils/apis';
import clsx from 'clsx';
import { UnzipOption } from 'bindings/UnzipOptions';

function generateZipText(tickedFiles : ClientFile[] | undefined, hoverFile : ClientFile | null) : string {
  // if there is no file ticked, and that there is a file, zip the current file to a zip file with the same name
  // if there is a file ticked, zip all the ticked files
  if (tickedFiles === undefined && hoverFile === undefined) {
    return "Zip";
  } else if (tickedFiles === undefined || tickedFiles.length === 0) {
    return `Zip to ${hoverFile?.name}.zip`;
  } else if (tickedFiles.length === 1) {
    return `Zip ${tickedFiles[0].name} to ${tickedFiles[0].name}.zip`;
  }
   else {
    const numFile = tickedFiles.length;
    return `Zip ${numFile} file${numFile > 1 ? "s" : ""} to Archive.zip`;
  }
}

function generateZipFileName(tickedFiles : ClientFile[], hoverFile : ClientFile | null) : string {
  if (tickedFiles === undefined && hoverFile === undefined) {
    return "";
  } else if (tickedFiles === undefined || tickedFiles.length === 0) {
    return hoverFile?.name + ".zip";
  } else if (tickedFiles.length === 1) {
    return tickedFiles[0].name + ".zip";
  }
   else {
    return "Archive.zip";
  }
}

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
      zipFiles,
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
      zipFiles: (files: ClientFile[], dest : string) => void;
      unzipFile: (file: ClientFile, unzipOption : UnzipOption) => void;
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
    const supportedZip = ["rar", "zip", "gz", "tgz"];

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

    const copyFile = async () => {
      if (tickedFiles?.includes(file as ClientFile)) {
        setClipboard(tickedFiles);
      } else {
        setClipboard([file as ClientFile]);
      }
      setTickedFiles([]);
      setClipboardAction('copy');
      toast.info('Files copied to clipboard');
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

    const unzip = async (unzipOption : UnzipOption) => {
      if (!supportedZip.includes(file?.name.split('.').pop() ?? ''))  { // not supported zip
        toast.error('Unsupported zip file type');
        setShowContextMenu(false);
      }
      else {
        unzipFile(file as ClientFile, unzipOption);
        setShowContextMenu(false); 
      }
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
            onClick={() => {
              copyFile();
              setShowContextMenu(false);
            }}
            disabled={file === null}
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
            label="Paste"
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
            disabled={!file}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            label="Delete"
            // iconComponent={<BackspaceIcon className="h-3.5 w-3.5 text-gray-300 opacity-50 group-hover:opacity-100" />}
            onClick={() => {
              deleteFile();
              setShowContextMenu(false);
            }}
            disabled={!file}
          />
          <ContextMenuButton
            className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
            
            label= {generateZipText(tickedFiles, file)}
            // disable if no file is ticked and if there is no file
            disabled={tickedFiles?.length === 0 && !file}
            onClick={() => {
              // if there is no file ticked, zip the current file to a zip file with the same name
              zipFiles(tickedFiles?.length === 0 ? [file! as ClientFile] : tickedFiles!, `${currentPath}/` + generateZipFileName(tickedFiles!, file));
              setTickedFiles([]);
              setShowContextMenu(false);
            }}
          />
          {supportedZip.includes(file?.name.split('.').pop() ?? '') && file?.file_type === "File" ? // if file name is null or file is not zip file
            (<>
            <ContextMenuButton
              className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
              label="Unzip here"
              onClick={() => {
                unzip("Normal");
              }}
              disabled={!file}
            />
            <ContextMenuButton
              className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
              label="Unzip here (smart)"
              onClick={() => {
                unzip("Smart");
              }}
              disabled={!file}
            />
            <ContextMenuButton
              className="truncate w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small font-medium"
              label={file ? 'Unzip to ' + file.name : ''}
              onClick={() => {
                unzip("ToDirectoryWithFileName");
              }}
              disabled={!file}
            />
            </>) : null}
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