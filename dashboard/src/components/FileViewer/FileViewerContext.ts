import { ClientFile } from 'bindings/ClientFile';
import { UnzipOption } from 'bindings/UnzipOption';
import { createContext, useContext } from 'react';

// Shared state and actions of a FileViewer, so FileList and FileContextMenu
// don't need every value passed down as a prop.
export interface FileViewerContextValue {
  /** the folder being shown, relative to the instance directory */
  path: string;
  directorySeparator: string;
  openedFile: ClientFile | null;
  tickedFiles: ClientFile[];
  setTickedFiles: (files: ClientFile[]) => void;
  tickFile: (file: ClientFile, ticked: boolean) => void;
  clipboard: ClientFile[];
  setClipboard: (files: ClientFile[]) => void;
  clipboardAction: 'copy' | 'cut';
  setClipboardAction: (action: 'copy' | 'cut') => void;
  pasteFiles: (destination: string) => Promise<void>;
  zipFiles: (files: ClientFile[], destination: string) => Promise<void>;
  unzipFile: (file: ClientFile, unzipOption: UnzipOption) => Promise<void>;
  /** ask the user to confirm, then delete the files */
  requestDelete: (files: ClientFile[]) => void;
  openCreateFileModal: (directory: string) => void;
  openCreateFolderModal: (directory: string) => void;
  openRenameModal: (filePath: string) => void;
}

export const FileViewerContext = createContext<FileViewerContextValue | null>(
  null
);

export const useFileViewer = () => {
  const context = useContext(FileViewerContext);
  if (!context)
    throw new Error('useFileViewer must be used inside a FileViewer');
  return context;
};
