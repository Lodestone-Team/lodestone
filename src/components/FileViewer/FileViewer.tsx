import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faFolder,
  faFloppyDisk,
  faDownload,
  faTrashCan,
  faUpload,
  faFilePen,
  faFolderPlus,
  faCaretDown,
  faListCheck,
  faArrowsRotate,
  faScissors,
  faPaste,
  faFileZipper,
} from '@fortawesome/free-solid-svg-icons';
import {
  Fragment,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { ClientFile } from 'bindings/ClientFile';
import { InstanceContext } from 'data/InstanceContext';
import { chooseFiles, parentPath } from 'utils/util';
import {
  deleteInstanceDirectory,
  deleteInstanceFile,
  downloadInstanceFile,
  saveInstanceFile,
  unzipInstanceFile,
  uploadInstanceFiles,
  moveInstanceFileOrDirectory,
  copyRecursive,
  zipInstanceFiles,
} from 'utils/apis';
import Button from 'components/Atoms/Button';
import { useElementSize, useLocalStorage } from 'usehooks-ts';
import ResizePanel from 'components/Atoms/ResizePanel';
import { Dialog, Menu, Transition } from '@headlessui/react';
import { useUserAuthorized } from 'data/UserInfo';
import { useQueryParam } from 'utils/hooks';
import { toast } from 'react-toastify';
import ErrorGraphic from 'components/ErrorGraphic';
import ConfirmDialog from '../Atoms/ConfirmDialog';
import FileList from './FileList';
import CreationModal from './CreationModal';
import CreateFolderForm from './CreateFolderForm';
import CreateFileForm from './CreateFileForm';
import RenameFileForm from './RenameFileForm';

import Breadcrumb from './Breadcrumb';
import { useFileContent, useFileList } from 'data/FileSystem';
import { FileEditor } from './FileEditor';
import { UnzipOption } from 'bindings/UnzipOptions';

export default function FileViewer() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const canRead = useUserAuthorized('can_read_instance_file', instance?.uuid);
  const canWrite = useUserAuthorized('can_write_instance_file', instance?.uuid);
  const queryClient = useQueryClient();
  const [path, setPath] = useQueryParam('path', '.');
  const [modalPath, setModalPath] = useState<string>('');
  const [openedFile, setOpenedFile] = useState<ClientFile | null>(null);
  const [createFileModalOpen, setCreateFileModalOpen] = useState(false);
  const [renameFileModalOpen, setRenameFileModalOpen] = useState(false);
  const [createFolderModalOpen, setCreateFolderModalOpen] = useState(false);
  const [deleteFileModalOpen, setDeleteFileModalOpen] = useState(false);
  const [dropping, setDropping] = useState(false);
  const [droppingDialog, setDroppingDialog] = useState(false);
  const [fileListSize, setFileListSize] = useLocalStorage('fileListSize', 200);
  const [tickedFiles, setTickedFiles] = useState<ClientFile[]>([]);
  const [clipboard, setClipboard] = useState<ClientFile[]>([]);
  const [clipboardAction, setClipboardAction] = useState<'copy' | 'cut'>('cut');
  const [fileContent, setFileContent] = useState('');
  const [
    boundingDivRef,
    { height: boundingDivHeight, width: boundingDivWidth },
  ] = useElementSize();

  const tickFile = (file: ClientFile, ticked: boolean) => {
    if (ticked) {
      setTickedFiles((files) => [...files, file]);
    } else {
      setTickedFiles((files) => files.filter((f) => f.path !== file.path));
    }
  };

  const atTopLevel = path === '.';
  let directorySeparator = '\\';
  // assume only linux paths contain /
  if (instance.path.includes('/')) directorySeparator = '/';

  /* Resets */
  useLayoutEffect(() => {
    setPath('.');
    setClipboard([]);
  }, [instance]);

  useLayoutEffect(() => {
    setOpenedFile(null);
    setTickedFiles([]);
  }, [path]);

  /* Query */

  const {
    data: fileList,
    isLoading: fileListLoading,
    error: fileListError,
  } = useFileList(instance.uuid, path, canRead);

  const {
    data: originalFileContent,
    isLoading: isFileLoading,
    error: fileError,
  } = useFileContent(instance.uuid, openedFile, canRead);

  useEffect(() => {
    setFileContent('');
  }, [openedFile]);

  const showingMonaco = openedFile && !isFileLoading && !fileError;
  /* Helper functions */

  const chooseFilesToUpload = async () => {
    const files = await chooseFiles();
    if (!files) return;
    // convert FileList to Array
    const fileArray = Array.from(files);
    await uploadInstanceFiles(instance.uuid, path, fileArray, queryClient);
  };

  const deleteSingleFile = async (file: ClientFile) => {
    if (file.file_type === 'Directory') {
      await deleteInstanceDirectory(
        instance.uuid,
        path,
        file.path,
        queryClient
      );
      tickFile(file, false);
      if (openedFile?.path.startsWith(file.path)) {
        setOpenedFile(null);
        setFileContent('');
      }
    } else if (file.file_type === 'File') {
      await deleteInstanceFile(instance.uuid, path, file, queryClient);
      tickFile(file, false);
      if (openedFile?.path === file.path) {
        setOpenedFile(null);
        setFileContent('');
      }
    }
  };
  const deleteTickedFiles = async () => {
    if (!tickedFiles) return;
    for (const file of tickedFiles) {
      await deleteSingleFile(file);
    }
    setTickedFiles([]);
  };

  const pasteFiles = async (currentPath: string) => {
    if (!clipboard) return;
    if (clipboardAction === 'copy') {
      const files: string[] = [];
      for (const file of clipboard) {
        files.push(file.path);
      }
      await copyRecursive(
        instance.uuid,
        {
          relative_paths_source: files,
          relative_path_dest: `${currentPath}`,
        },
        directorySeparator,
        queryClient
      );
    } else if (clipboardAction === 'cut') {
      for (const file of clipboard) {
        console.log(
          'moving',
          file.path,
          `${currentPath} | ${directorySeparator} | ${file.name}`
        );
        await moveInstanceFileOrDirectory(
          instance.uuid,
          file.path,
          `${currentPath}${directorySeparator}${file.name}`,
          queryClient,
          directorySeparator
        );
        if (openedFile?.path.startsWith(file.path)) {
          setOpenedFile(null);
          setFileContent('');
        }
      }
    }
    setClipboard([]);
  };

  const downloadTickedFiles = async () => {
    if (!tickedFiles) return;
    
    for (const file of tickedFiles) {
      const timeout = file?.file_type == 'Directory' ? 60000 : 5000;
      if (file.file_type === 'Directory') {
        toast.info('Zipping directory for download...');
      }
      
      downloadInstanceFile(instance.uuid, file, timeout);
      tickFile(file, false);
    }
  };

  const zipFiles = async (files: ClientFile[], dest: string) => {
    await zipInstanceFiles(instance.uuid, {
      target_relative_paths: files.map((f) => f.path),
      destination_relative_path: dest,
    });
  };

  const unzipFile = async (file: ClientFile, unzipOption: UnzipOption) => {
    if (file.file_type !== 'File') {
      toast.error('Only files can be unzipped');
      return;
    }

    await unzipInstanceFile(instance.uuid, file, unzipOption);
  };

  const unzipTickedFile = async () => {
    if (!tickedFiles) return;
    const file = tickedFiles[0];
    await unzipFile(file, 'Smart');
    tickFile(file, false);
  };

  const handleFileDrop = async (event: React.DragEvent) => {
    if (!event.dataTransfer.types.includes('Files')) return;

    for (const i in event.dataTransfer.items) {
      const item = event.dataTransfer.items[i];
      if (item.kind !== 'file') continue;
      const entry =
        'getAsEntry' in DataTransferItem.prototype
          ? (item as any).getAsEntry()
          : item.webkitGetAsEntry();
      if (entry.isDirectory) {
        toast.error('Only files can be uploaded');
        setDropping(false);
        setDroppingDialog(false);
        return;
      }
    }
    uploadInstanceFiles(
      instance.uuid,
      path,
      Array.from(event.dataTransfer?.files),
      queryClient
    );
    setDropping(false);
    setDroppingDialog(false);
  };

  /* UI */

  const fileCheckIcon = (
    <svg
      viewBox="0 0 512 512"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      focusable="false"
      aria-hidden="true"
      className={`svg-inline--fa w-4 opacity-50`}
    >
      <path
        d="M453.8 149.8L304 0V99.8C304 127.4 326.4 149.8 354 149.8H453.8Z"
        fill="currentColor"
      />
      <path
        d="M306 197.8C278.4 197.8 256 175.4 256 147.8V0H106C79.6002 0 58.2002 21.4 58.2002 47.8V464.1C58.2002 490.5 79.6002 511.9 106 511.9H406C432.4 511.9 453.8 490.5 453.8 464.1V197.8H306ZM383.8 354.9C383.8 368.2 373.1 378.9 359.8 378.9H280V458.7C280 472 269.3 482.7 256 482.7C242.7 482.7 232 472 232 458.7V378.9H152.2C138.9 378.9 128.2 368.2 128.2 354.9C128.2 341.6 138.9 330.9 152.2 330.9H232V251.1C232 237.8 242.7 227.1 256 227.1C269.3 227.1 280 237.8 280 251.1V330.9H359.8C373 330.9 383.8 341.7 383.8 354.9Z"
        fill="currentColor"
      />
    </svg>
  );

  return (
    <>
      <CreationModal
        setModalOpen={setCreateFolderModalOpen}
        modalOpen={createFolderModalOpen}
      >
        <CreateFolderForm
          onCancel={() => setCreateFolderModalOpen(false)}
          onSuccess={() => setCreateFolderModalOpen(false)}
          fileList={fileList}
          path={modalPath}
        />
      </CreationModal>
      <CreationModal
        setModalOpen={setCreateFileModalOpen}
        modalOpen={createFileModalOpen}
      >
        <CreateFileForm
          onCancel={() => setCreateFileModalOpen(false)}
          onSuccess={() => setCreateFileModalOpen(false)}
          path={modalPath}
          fileList={fileList}
        />
      </CreationModal>
      <CreationModal
        setModalOpen={setRenameFileModalOpen}
        modalOpen={renameFileModalOpen}
      >
        <RenameFileForm
          onCancel={() => setRenameFileModalOpen(false)}
          onSuccess={() => setRenameFileModalOpen(false)}
          path={modalPath}
        />
      </CreationModal>
      <ConfirmDialog
        isOpen={deleteFileModalOpen}
        onClose={() => setDeleteFileModalOpen(false)}
        onConfirm={async () => {
          setDeleteFileModalOpen(false);
          deleteTickedFiles();
        }}
        title="Delete file(s)"
        type="danger"
      >
        Are you sure you want to delete the following?
        <ul className="list-inside list-disc">
          {tickedFiles.map((file) => (
            <li key={file.path}>
              {file.name} {file.file_type != 'File' && `(${file.file_type})`}
            </li>
          ))}
        </ul>
      </ConfirmDialog>
      <div
        ref={boundingDivRef}
        className="relative flex h-full w-full grow flex-col gap-3"
        onDragEnter={(e) => {
          e.preventDefault();
          e.dataTransfer.types.includes('Files') && setDropping(true);
          e.stopPropagation();
        }}
        onDragLeave={(e) => {
          e.preventDefault();
          setDropping(false);
          e.stopPropagation();
        }}
        onDragOver={(e) => {
          e.preventDefault();
          e.stopPropagation();
        }}
        onDrop={(e: React.DragEvent) => {
          e.preventDefault();
          handleFileDrop(e);
          e.stopPropagation();
        }}
      >
        <Dialog
          open={dropping || droppingDialog}
          onClose={() => {
            return;
          }}
        >
          <div
            onDragEnter={(e) => {
              e.preventDefault();
              e.dataTransfer.types.includes('Files') && setDroppingDialog(true);
              e.stopPropagation();
            }}
            onDragLeave={(e) => {
              e.preventDefault();
              setDroppingDialog(false);
              e.stopPropagation();
            }}
            onDragOver={(e) => {
              e.preventDefault();
              e.stopPropagation();
            }}
            onDrop={(e: React.DragEvent) => {
              e.preventDefault();
              handleFileDrop(e);
              e.stopPropagation();
            }}
          >
            <div className="fixed inset-0 bg-[#000]/80" />
            <div className="fixed inset-0 overflow-y-auto">
              <div className="pointer-events-none flex min-h-full items-center justify-center p-4 text-center text-white/50">
                <Dialog.Panel className="pointer-events-none  flex w-[200px] flex-col items-center justify-center gap-4 rounded-3xl border-2 border-dashed border-gray-faded/30 bg-gray-800 bg-opacity-50 pb-8 pt-12">
                  <FontAwesomeIcon
                    className="pointer-events-none m-0 p-0 text-h1 text-gray-faded/80"
                    icon={faDownload}
                  />
                  <p className="pointer-events-none">Release to Upload File</p>
                </Dialog.Panel>
              </div>
            </div>
          </div>
        </Dialog>
        <div className="flex flex-row items-center justify-between gap-4">
          <Menu as="div" className="relative inline-block text-left">
            <Menu.Button
              as={Button}
              label="Add/Remove"
              icon={faCaretDown}
              disabled={!fileList}
            ></Menu.Button>
            <Transition
              as={Fragment}
              enter="transition ease-out duration-200"
              enterFrom="opacity-0 -translate-y-1"
              enterTo="opacity-100 translate-y-0"
              leave="transition ease-in duration-150"
              leaveFrom="opacity-100 translate-y-0"
              leaveTo="opacity-0 -translate-y-1"
            >
              <Menu.Items className="absolute -left-0.5 z-10 mt-2 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-none">
                <div className="py-2 px-1.5">
                  <Menu.Item disabled={!fileList}>
                    {({ active, disabled }) => (
                      <Button
                        label={
                          tickedFiles.length === fileList?.length
                            ? 'Deselect all'
                            : 'Select all'
                        }
                        className="w-full whitespace-nowrap py-1.5"
                        onClick={() => {
                          if (!fileList) return;
                          if (tickedFiles.length === fileList.length) {
                            setTickedFiles([]);
                          } else {
                            setTickedFiles([...fileList]);
                          }
                        }}
                        icon={faListCheck}
                        variant="text"
                        align="start"
                        disabled={disabled}
                      />
                    )}
                  </Menu.Item>
                  <Menu.Item disabled={tickedFiles.length === 0 || !canWrite}>
                    {({ active, disabled }) => (
                      <Button
                        className="w-full whitespace-nowrap py-1.5"
                        label="Cut selected"
                        icon={faScissors}
                        onClick={() => {
                          setClipboard(tickedFiles);
                          setClipboardAction('cut');
                          setTickedFiles([]);
                          toast.info(
                            `${tickedFiles.length} ${
                              tickedFiles.length === 1 ? 'file' : 'files'
                            } cut to clipboard`
                          );
                        }}
                        variant="text"
                        align="start"
                        disabled={disabled}
                      />
                    )}
                  </Menu.Item>
                  <Menu.Item disabled={tickedFiles.length === 0 || !canRead}>
                    {({ active, disabled }) => (
                      <Button
                        className="w-full whitespace-nowrap py-1.5"
                        label="Download selected"
                        icon={faDownload}
                        onClick={downloadTickedFiles}
                        variant="text"
                        align="start"
                        disabled={disabled}
                      />
                    )}
                  </Menu.Item>
                  <Menu.Item disabled={tickedFiles.length !== 1 || !canWrite}>
                    {({ active, disabled }) => (
                      <Button
                        className="w-full whitespace-nowrap py-1.5"
                        label="Unarchive selected"
                        icon={faFileZipper}
                        onClick={unzipTickedFile}
                        variant="text"
                        align="start"
                        disabled={disabled}
                      />
                    )}
                  </Menu.Item>
                </div>
                <div className="py-2 px-1.5">
                  <Menu.Item disabled={!canWrite}>
                    {({ active, disabled }) => (
                      <Button
                        label="New file"
                        className="w-full whitespace-nowrap py-1.5"
                        onClick={() => {
                          setModalPath(path);
                          setCreateFileModalOpen(true);
                        }}
                        iconComponent={fileCheckIcon}
                        variant="text"
                        align="start"
                        disabled={disabled}
                      />
                    )}
                  </Menu.Item>
                  <Menu.Item disabled={!canWrite}>
                    {({ active, disabled }) => (
                      <Button
                        label="New folder"
                        className="w-full whitespace-nowrap py-1.5"
                        onClick={() => {
                          setModalPath(path);
                          setCreateFolderModalOpen(true);
                        }}
                        icon={faFolderPlus}
                        variant="text"
                        align="start"
                        disabled={disabled}
                      />
                    )}
                  </Menu.Item>
                </div>
                <div className="py-2 px-1.5">
                  <Menu.Item disabled={tickedFiles.length === 0 || !canWrite}>
                    {({ active, disabled }) => (
                      <Button
                        label="Delete selected"
                        className="w-full whitespace-nowrap py-1.5"
                        onClick={() => {
                          setModalPath(path);
                          setDeleteFileModalOpen(true);
                        }}
                        icon={faTrashCan}
                        variant="text"
                        align="start"
                        intention="danger"
                        disabled={disabled}
                      />
                    )}
                  </Menu.Item>
                </div>
              </Menu.Items>
            </Transition>
          </Menu>

          <Breadcrumb
            path={path}
            openedFile={openedFile}
            setPath={setPath}
            directorySeparator={directorySeparator}
          />

          {clipboard.length !== 0 && (
            <Button
              className="h-fit whitespace-nowrap"
              label={`Paste ${clipboard.length} ${
                clipboard.length > 1 ? 'files' : 'file'
              }`}
              icon={faPaste}
              onClick={() => pasteFiles(path)}
            />
          )}
          {showingMonaco && (
            <>
              <Button
                className="h-fit"
                label="Save"
                icon={faFloppyDisk}
                onClick={() =>
                  saveInstanceFile(
                    instance.uuid,
                    path,
                    openedFile,
                    fileContent,
                    queryClient
                  )
                }
                disabled={
                  !openedFile ||
                  fileContent === originalFileContent ||
                  !showingMonaco
                }
              />
              <Button
                className="h-fit whitespace-nowrap"
                label="Discard Changes"
                icon={faArrowsRotate}
                onClick={() => setFileContent(originalFileContent || '')}
                disabled={
                  !openedFile ||
                  fileContent === originalFileContent ||
                  !showingMonaco
                }
              />
            </>
          )}
          <Button
            className="h-fit"
            label="Upload"
            icon={faUpload}
            onClick={chooseFilesToUpload}
            disabled={!canWrite}
          />
        </div>

        {canRead ? (
          <div
            className="flex h-full w-full grow flex-row divide-x divide-gray-faded/30 rounded-lg border border-gray-faded/30 bg-gray-800"
            onDragEnter={(e) => {
              e.preventDefault();
              e.dataTransfer.types.includes('Files') && setDropping(true);
              //setDropping(true);
              e.stopPropagation();
            }}
            onDragLeave={(e) => {
              e.preventDefault();
              setDropping(false);
              e.stopPropagation();
            }}
            onDragOver={(e) => {
              e.preventDefault();
            }}
            onDrop={(e: React.DragEvent) => {
              e.preventDefault();
              handleFileDrop(e);
              e.stopPropagation();
            }}
          >
            <ResizePanel
              direction="e"
              maxSize={boundingDivWidth - 200}
              minSize={200}
              size={fileListSize}
              validateSize={false}
              onResize={setFileListSize}
              containerClassNames="grow shrink-0 rounded-l-lg last:rounded-r-lg overflow-clip"
              grow={!openedFile}
            >
              <FileList
                path={path}
                atTopLevel={atTopLevel}
                fileList={fileList}
                clipboard={clipboard}
                loading={fileListLoading}
                error={fileListError}
                tickedFiles={tickedFiles}
                tickFile={tickFile}
                zipFiles={zipFiles}
                unzipFile={unzipFile}
                pasteFiles={pasteFiles}
                openedFile={openedFile}
                onParentClick={() =>
                  setPath(parentPath(path, directorySeparator), false)
                }
                onEmptyClick={() => {
                  setOpenedFile(null);
                  setTickedFiles([]);
                }}
                onFileClick={(file) => {
                  if (file.file_type === 'Directory') {
                    setPath(file.path, false);
                  } else {
                    setOpenedFile(file);
                  }
                }}
                setCreateFileModalOpen={setCreateFileModalOpen}
                setCreateFolderModalOpen={setCreateFolderModalOpen}
                setRenameFileModalOpen={setRenameFileModalOpen}
                setModalPath={setModalPath}
                setClipboard={setClipboard}
                clipboardAction={clipboardAction}
                setClipboardAction={setClipboardAction}
                setTickedFiles={setTickedFiles}
                deleteSingleFile={deleteSingleFile}
                deleteTickedFiles={deleteTickedFiles}
              />
            </ResizePanel>
            {openedFile && (
              <div className="min-w-0 grow">
                <div className="h-full">
                  {showingMonaco ? (
                    <FileEditor
                      path={path}
                      monacoPath={
                        instance.path + directorySeparator + openedFile?.path ||
                        ''
                      }
                      file={openedFile}
                      originalFileContent={originalFileContent}
                      fileContent={fileContent}
                      setFileContent={setFileContent}
                    />
                  ) : (
                    <ErrorGraphic
                      icon={faFilePen}
                      message="File Editor"
                      message2={
                        fileError
                          ? fileError?.message ?? 'Unknown Error'
                          : isFileLoading
                          ? 'Loading...'
                          : 'Select a file to view its contents'
                      }
                      className=""
                      iconClassName="text-gray-500"
                      messageClassName="text-gray-400"
                    />
                  )}
                </div>
              </div>
            )}
          </div>
        ) : (
          <ErrorGraphic
            icon={faFolder}
            message="You don't have permission to read this folder"
            className="text-clip rounded-lg border border-gray-faded/30"
            iconClassName="text-gray-400"
            messageClassName="text-white/50"
          />
        )}
        <div className="absolute bottom-0 left-0 flex translate-y-full flex-row gap-4 px-4 py-2 text-medium font-medium text-white/50">
          {tickedFiles.length === 1 && <div>1 item selected</div>}
          {tickedFiles.length > 1 && (
            <div>{tickedFiles.length} items selected</div>
          )}
          {clipboard.length === 1 && <div>1 item in clipboard</div>}
          {clipboard.length > 1 && (
            <div>{clipboard.length} items in clipboard</div>
          )}
        </div>
      </div>
    </>
  );
}
