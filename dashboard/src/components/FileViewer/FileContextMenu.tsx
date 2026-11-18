import React, { forwardRef } from 'react';
import ContextMenuButton from 'components/Atoms/ContextMenuButton';
import { toast } from 'react-toastify';
import { ClientFile } from 'bindings/ClientFile';
import { UnzipOption } from 'bindings/UnzipOption';
import { joinPath, supportedZip } from 'utils/util';
import { useFileViewer } from './FileViewerContext';
import { useT } from 'i18n';
import { MonoText } from 'components/Atoms/Values';

function generateZipFileName(
  tickedFiles: ClientFile[],
  hoverFile: ClientFile | null
): string {
  if (tickedFiles === undefined && hoverFile === undefined) {
    return '';
  } else if (tickedFiles === undefined || tickedFiles.length === 0) {
    return hoverFile?.name + '.zip';
  } else if (tickedFiles.length === 1) {
    return tickedFiles[0].name + '.zip';
  } else {
    return 'Archive.zip';
  }
}

const FileContextMenu = forwardRef(
  (
    {
      file,
      coords,
      setShowContextMenu,
    }: {
      file: ClientFile | null;
      coords: { x: number; y: number };
      setShowContextMenu: (showContextMenu: boolean) => void;
    },
    ref: React.Ref<HTMLDivElement>
  ) => {
    // re-render on a language change; generateZipText reads it too
    const t = useT();
    const {
      path: currentPath,
      directorySeparator,
      tickedFiles,
      setTickedFiles,
      clipboard,
      setClipboard,
      setClipboardAction,
      pasteFiles,
      zipFiles,
      unzipFile,
      requestDelete,
      openCreateFileModal,
      openCreateFolderModal,
      openRenameModal,
    } = useFileViewer();

    // right-clicking a ticked file acts on all ticked files, otherwise just on
    // the clicked file. Compare paths: the objects are replaced on refetch.
    const selection = (): ClientFile[] => {
      if (!file) return [];
      return tickedFiles.some((f) => f.path === file.path)
        ? tickedFiles
        : [file];
    };

    // new files and folders go into the right-clicked folder, or else the
    // folder being shown
    const targetDirectory =
      file?.file_type === 'Directory' ? file.path : currentPath;

    const deleteFile = () => {
      // asks for confirmation first, like the toolbar's Delete
      requestDelete(selection());
    };

    const copyOrCutFile = (action: 'copy' | 'cut') => {
      const files = selection();
      setClipboard(files);
      setTickedFiles([]);
      setClipboardAction(action);
      toast.info(
        action === 'copy'
          ? t('files.contextMenu.copiedToClipboard', { count: files.length })
          : t('files.contextMenu.cutToClipboard', { count: files.length })
      );
    };

    const unzip = async (unzipOption: UnzipOption) => {
      if (!supportedZip.includes(file?.extension ?? '')) {
        // not supported zip
        toast.error(t('files.contextMenu.unsupportedZip'));
        setShowContextMenu(false);
      } else {
        unzipFile(file as ClientFile, unzipOption);
        setShowContextMenu(false);
      }
    };

    return (
      <div
        // as wide as its longest item, within limits; FileList measures it
        className="fixed right-0 z-50 mt-1.5 w-max max-w-64 min-w-44 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-900 drop-shadow-md focus:outline-hidden"
        style={{
          top: coords.y + 'px',
          left: coords.x + 'px',
          position: 'absolute',
        }}
        ref={ref}
      >
        {/* what the menu acts on, named once */}
        {(tickedFiles.length > 0 || file) && (
          <div className="max-w-64 truncate px-2.5 py-1.5 font-mono text-small text-white/50">
            {tickedFiles.length > 1 ? (
              <span className="font-sans">
                {t('files.contextMenu.items', { count: tickedFiles.length })}
              </span>
            ) : (
              <MonoText text={(tickedFiles[0] ?? file)?.name ?? ''} max={32} />
            )}
          </div>
        )}
        <div className="py-1">
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
            label={t('common.copy')}
            onClick={() => {
              copyOrCutFile('copy');
              setShowContextMenu(false);
            }}
            disabled={file === null}
          />
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
            label={t('files.contextMenu.cut')}
            onClick={() => {
              copyOrCutFile('cut');
              setShowContextMenu(false);
            }}
            disabled={file === null}
          />
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
            label={
              file?.file_type !== 'Directory'
                ? t('files.contextMenu.paste')
                : t('files.contextMenu.pasteInto')
            }
            caption={file?.file_type === 'Directory' ? file.name : undefined}
            onClick={() => {
              pasteFiles(targetDirectory);
              setShowContextMenu(false);
            }}
            disabled={clipboard.length === 0}
          />
        </div>
        <div className="py-1">
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
            label={t('common.rename')}
            onClick={() => {
              if (file) openRenameModal(file.path);
              setShowContextMenu(false);
            }}
            disabled={!file}
          />
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap text-red-200"
            label={t('common.delete')}
            onClick={() => {
              deleteFile();
              setShowContextMenu(false);
            }}
            disabled={!file}
          />
        </div>
        <div className="py-1">
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
            label={t('files.contextMenu.zip')}
            caption={
              tickedFiles.length > 0 || file
                ? generateZipFileName(tickedFiles, file)
                : undefined
            }
            // disable if no file is ticked and if there is no file
            disabled={tickedFiles.length === 0 && !file}
            onClick={() => {
              // if there is no file ticked, zip the current file to a zip file with the same name
              zipFiles(
                tickedFiles.length === 0 ? [file as ClientFile] : tickedFiles,
                joinPath(
                  currentPath,
                  generateZipFileName(tickedFiles, file),
                  directorySeparator
                )
              );
              setTickedFiles([]);
              setShowContextMenu(false);
            }}
          />
          {supportedZip.includes(file?.extension ?? '') &&
          file?.file_type === 'File' ? ( // if file name is null or file is not zip file
            <>
              <ContextMenuButton
                className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
                label={t('files.contextMenu.unzipHere')}
                onClick={() => {
                  unzip('Normal');
                }}
                disabled={!file}
              />
              <ContextMenuButton
                className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
                label={t('files.contextMenu.unzipHereSmart')}
                onClick={() => {
                  unzip('Smart');
                }}
                disabled={!file}
              />
              <ContextMenuButton
                className="w-full truncate rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
                label={t('files.contextMenu.unzipTo')}
                caption={file.file_stem}
                onClick={() => {
                  unzip('ToDirectoryWithFileName');
                }}
                disabled={!file}
              />
            </>
          ) : null}
        </div>
        <div className="py-1">
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
            label={t('files.newFolder')}
            onClick={() => {
              openCreateFolderModal(targetDirectory);
              setShowContextMenu(false);
            }}
          />
          <ContextMenuButton
            className="w-full rounded-none bg-gray-900 px-2.5 text-small font-medium whitespace-nowrap"
            label={t('files.newFile')}
            onClick={() => {
              openCreateFileModal(targetDirectory);
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
