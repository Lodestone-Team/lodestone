export default {
  // FileViewer: the page, its toolbar and dialogs
  /** the page title, inside common.pageTitle */
  'files.pageTitle': 'Server File Viewer',
  /** shown while files are dragged over the file viewer */
  'files.dropToUpload': 'Release to Upload File',
  /** the toolbar menu with the actions below */
  'files.toolbar.addRemove': 'Add/Remove',
  'files.toolbar.selectAll': 'Select all',
  'files.toolbar.deselectAll': 'Deselect all',
  'files.toolbar.cutSelected': 'Cut selected',
  'files.toolbar.downloadSelected': 'Download selected',
  'files.toolbar.unarchiveSelected': 'Unarchive selected',
  'files.toolbar.deleteSelected': 'Delete selected',
  /** pastes the files in the clipboard into the folder being shown */
  'files.toolbar.paste': {
    one: 'Paste {count} file',
    other: 'Paste {count} files',
  },
  /** reverts the edits in the file editor */
  'files.toolbar.discardChanges': 'Discard Changes',
  'files.toolbar.upload': 'Upload',
  /** a toast after “Cut selected” */
  'files.cutToClipboard': {
    one: '{count} file cut to clipboard',
    other: '{count} files cut to clipboard',
  },
  /** a toast: a folder is compressed before it is downloaded */
  'files.zippingForDownload': 'Zipping directory for download…',
  /** {name} is a file name, {error} the reason */
  'files.downloadFailed': 'Failed to download {name}: {error}',
  'files.onlyFilesUnzip': 'Only files can be unzipped',
  /** a toast when a folder is dropped to upload it */
  'files.onlyFilesUpload': 'Only files can be uploaded',
  /** the title of the dialog confirming a deletion */
  'files.delete.title': { one: 'Delete file', other: 'Delete {count} files' },
  /** followed by the list of files and folders */
  'files.delete.message': 'Are you sure you want to delete the following?',
  /** after a folder’s name in the list */
  'files.delete.directory': '(Directory)',
  /** after the name of an entry that is neither a file nor a folder */
  'files.delete.unknownType': '(Unknown)',
  /** for screen readers, while a file loads */
  'files.loadingFile': 'Loading file',
  /** shown in the editor pane when no file can be shown */
  'files.editor.title': 'File Editor',
  'files.editor.selectFile': 'Select a file to view its contents',
  'files.noReadPermission': 'You don’t have permission to read this folder',
  /** below the file list: files and folders selected */
  'files.status.selected': {
    one: '{count} item selected',
    other: '{count} items selected',
  },
  /** below the file list: files and folders copied or cut */
  'files.status.inClipboard': {
    one: '{count} item in clipboard',
    other: '{count} items in clipboard',
  },

  // FileList: the list of files and folders
  /** for screen readers, while a folder loads */
  'files.list.loading': 'Loading files',
  'files.list.empty': 'No files here…',
  /** in the modification time column */
  'files.list.unknownTime': 'Unknown Creation Time',
  /** alt text of an icon */
  'files.list.folderIcon': 'folder icon',
  /** alt text of an icon */
  'files.list.fileIcon': 'file icon',

  // FileContextMenu: the right-click menu of the file list
  'files.contextMenu.cut': 'Cut',
  'files.contextMenu.paste': 'Paste',
  /** {name} is a folder name */
  'files.contextMenu.pasteInto': 'Paste into {name}',
  /** compresses files into a .zip archive */
  'files.contextMenu.zip': 'Zip',
  /** {archive} is the archive’s file name, e.g. “world.zip” */
  'files.contextMenu.zipTo': 'Zip to {archive}',
  /** {name} is a file name, {archive} the archive’s file name */
  'files.contextMenu.zipFile': 'Zip {name} to {archive}',
  /** {archive} is the archive’s file name, “Archive.zip” */
  'files.contextMenu.zipFiles': {
    one: 'Zip {count} file to {archive}',
    other: 'Zip {count} files to {archive}',
  },
  /** extracts the archive into the folder being shown */
  'files.contextMenu.unzipHere': 'Unzip here',
  /** extracts the archive, without its single top-level folder if it has one */
  'files.contextMenu.unzipHereSmart': 'Unzip here (smart)',
  /** {name} is the folder the archive is extracted to (its name without the extension) */
  'files.contextMenu.unzipTo': 'Unzip to {name}',
  /** a toast after Copy: files and folders */
  'files.contextMenu.copiedToClipboard': {
    one: '{count} item copied to clipboard',
    other: '{count} items copied to clipboard',
  },
  /** a toast after Cut: files and folders */
  'files.contextMenu.cutToClipboard': {
    one: '{count} item cut to clipboard',
    other: '{count} items cut to clipboard',
  },
  'files.contextMenu.unsupportedZip': 'Unsupported zip file type',
  /** in the toolbar menu and the right-click menu */
  'files.newFolder': 'New folder',
  /** in the toolbar menu and the right-click menu */
  'files.newFile': 'New file',

  // CreateFileForm, CreateFolderForm, RenameFileForm: the dialogs
  'files.createFile.label': 'Name your file',
  /** the placeholder of the file name field */
  'files.createFile.placeholder': 'Untitled',
  'files.createFile.submit': 'Create file',
  'files.createFolder.label': 'Name your folder',
  /** the placeholder of the folder name field */
  'files.createFolder.placeholder': 'Untitled folder',
  'files.createFolder.submit': 'Create folder',
  /** the new name of a file or folder */
  'files.rename.label': 'Name your file',
  'files.rename.submit': 'Rename file',

  // FileEditor
  /** for screen readers, while the code editor starts */
  'files.editor.loading': 'Loading editor',
} as const;
