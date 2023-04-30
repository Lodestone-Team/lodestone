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
  useState,
} from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { ClientFile } from 'bindings/ClientFile';
import { InstanceContext } from 'data/InstanceContext';
import {
  chooseFiles,
  parentPath,
} from 'utils/util';
import {
  deleteInstanceDirectory,
  deleteInstanceFile,
  downloadInstanceFiles,
  saveInstanceFile,
  unzipInstanceFile,
  uploadInstanceFiles,
  moveInstanceFileOrDirectory,
} from 'utils/apis';
import Button from 'components/Atoms/Button';
import { useLocalStorage } from 'usehooks-ts';
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
  const [fileListSize, setFileListSize] = useLocalStorage('fileListSize', 200);
  const [tickedFiles, setTickedFiles] = useState<ClientFile[]>([]);
  const [clipboard, setClipboard] = useState<ClientFile[]>([]);
  const [clipboardAction, setClipboardAction] = useState<'copy' | 'cut'>('cut');
  const [fileContent, setFileContent] = useState('');
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
  }
  const deleteTickedFiles = async () => {
    if (!tickedFiles) return;
    for (const file of tickedFiles) {
      await deleteSingleFile(file);
    }
    setTickedFiles([]);
  };

  const pasteFiles = async () => {
    if (!clipboard) return;
    if (clipboardAction === 'copy')
      throw new Error('copying files is not implemented yet');
    if (clipboardAction === 'cut') {
      for (const file of clipboard) {
        console.log(
          'moving',
          file.path,
          `${path} | ${directorySeparator} | ${file.name}`
        );
        await moveInstanceFileOrDirectory(
          instance.uuid,
          file.path,
          `${path}${directorySeparator}${file.name}`,
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
    const missedDirectories: string[] = [];
    for (const file of tickedFiles) {
      if (file.file_type === 'Directory') {
        missedDirectories.push(file.path);
      } else if (file.file_type === 'File') {
        downloadInstanceFiles(instance.uuid, file);
        tickFile(file, false);
      }
    }
    if (missedDirectories.length > 0) {
      const missedDirectoriesString = missedDirectories.join(', ');
      toast.error(
        `Downloading a directory is not supported. The following directories were not downloaded: ${missedDirectoriesString}`
      );
    }
  };

  const unzipFile = async (file: ClientFile)  => { 
    if (file.file_type !== 'File') {
      toast.error('Only files can be unzipped');
      return;
    }

    await unzipInstanceFile(
      instance.uuid,
      file,
      path,
      queryClient,
      directorySeparator
    );
  }

  const unzipTickedFile = async () => {
    if (!tickedFiles) return;
    const file = tickedFiles[0];
    await unzipFile(file);
    tickFile(file, false);
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
      <div className="relative flex h-full w-full grow flex-col gap-3">
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
                          toast.info('Files cut to clipboard');
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
                          setCreateFileModalOpen(true)
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
              onClick={pasteFiles}
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
          <div className="flex h-full w-full grow flex-row divide-x divide-gray-faded/30 rounded-lg border border-gray-faded/30 bg-gray-800">
            <ResizePanel
              direction="e"
              maxSize={500}
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
                loading={fileListLoading}
                error={fileListError}
                tickedFiles={tickedFiles}
                tickFile={tickFile}
                unzipFile={unzipFile}
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
          {tickedFiles.length === 1 && (
            <div>1 item selected</div>
          )}
          {tickedFiles.length > 1 && (
            <div>{tickedFiles.length} items selected</div>
          )}
          {clipboard.length === 1 && (
            <div>1 item in clipboard</div>
          )}
          {clipboard.length > 1 && (
            <div>{clipboard.length} items in clipboard</div>
          )}
        </div>
      </div>
    </>
  );
}
