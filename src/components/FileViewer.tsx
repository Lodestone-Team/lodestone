import Editor, { useMonaco } from '@monaco-editor/react';
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faFolder,
  faFile,
  faClipboardQuestion,
  faFloppyDisk,
  faDownload,
  faTrashCan,
  faCaretDown,
  faUpload,
  faFilePen,
} from '@fortawesome/free-solid-svg-icons';
import { Fragment, useContext, useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ClientFile } from 'bindings/ClientFile';
import { InstanceContext } from 'data/InstanceContext';
import axios from 'axios';
import { FileType } from 'bindings/FileType';
import { axiosWrapper, catchAsyncToString, formatTimeAgo } from 'utils/util';
import Button from 'components/Atoms/Button';
import { useLocalStorage } from 'usehooks-ts';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik } from 'formik';
import ResizePanel from 'components/Atoms/ResizePanel';
import { Menu, Transition } from '@headlessui/react';
import { faSquare } from '@fortawesome/free-regular-svg-icons';
import clsx from 'clsx';

type Monaco = typeof monaco;

const fileTypeToIconMap: Record<FileType, React.ReactElement> = {
  Directory: <FontAwesomeIcon icon={faFolder} className="text-blue-accent" />,
  File: <FontAwesomeIcon icon={faFile} className="text-gray-400" />,
  Unknown: (
    <FontAwesomeIcon icon={faClipboardQuestion} className="text-ochre" />
  ),
};

const fileSorter = (a: ClientFile, b: ClientFile) => {
  if (a.file_type === b.file_type) {
    return a.name.localeCompare(b.name);
  }
  return a.file_type.localeCompare(b.file_type);
};

const useFileList = (uuid: string, path: string) =>
  useQuery<ClientFile[], Error>(
    ['instance', uuid, 'fileList', path],
    () => {
      return axiosWrapper<ClientFile[]>({
        url: `/instance/${uuid}/fs/ls/${path}`,
        method: 'GET',
      }).then((response) => {
        // sort by file type, then file name
        return response.sort(fileSorter);
      });
    },
    {
      retry: false,
      cacheTime: 0,
      staleTime: 0,
    }
  );

const useFileContent = (uuid: string, targetFile: ClientFile | null) =>
  useQuery<string, Error>(
    ['instance', uuid, 'fileContent', targetFile?.path],
    () => {
      return axiosWrapper<string>({
        url: `/instance/${uuid}/fs/read/${targetFile?.path}`,
        method: 'GET',
        transformResponse: (data) => data,
      }).then((response) => {
        return response;
      });
    },
    {
      enabled: targetFile !== null,
      cacheTime: 0,
      staleTime: 0,
      retry: false,
    }
  );

export default function FileViewer() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  const monaco = useMonaco();
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  if (!instance) throw new Error('No instance selected');
  const [path, setPath] = useState('');
  const [targetFile, setTargetFile] = useState<ClientFile | null>(null);
  const [edittedFileContent, setEdittedFileContent] = useState('');
  const [fileListSize, setFileListSize] = useLocalStorage('fileListSize', 200);
  const queryClient = useQueryClient();
  const atTopLevel = path === '';
  let direcotrySeparator = '\\';
  // assume only linux paths contain /
  if (instance.path.includes('/')) direcotrySeparator = '/';

  const parentPath = (path: string) => {
    const pathParts = path.split(direcotrySeparator);
    pathParts.pop();
    return pathParts.join(direcotrySeparator);
  };

  /* Query */

  const {
    data: fileList,
    isLoading: fileListLoading,
    error: fileListError,
  } = useFileList(instance.uuid, path);

  const {
    data: originalFileContent,
    isLoading: isFileLoading,
    error: fileError,
  } = useFileContent(instance.uuid, targetFile);

  useEffect(() => {
    setEdittedFileContent(originalFileContent || '');
  }, [originalFileContent]);

  useEffect(() => {
    setEdittedFileContent('');
  }, [targetFile]);

  /* Monaco */

  function handleEditorDidMount(
    editor: monaco.editor.IStandaloneCodeEditor,
    monaco: Monaco
  ) {
    editorRef.current = editor;
  }

  // hack to get .lodestone_config detected as json
  const monacoPath =
    targetFile?.name === '.lodestone_config'
      ? '.lodestone_config.json'
      : targetFile?.name;

  const showingMonaco = targetFile && !isFileLoading && !fileError;

  useEffect(() => {
    // set monaco theme, just a different background color
    if (monaco) {
      monaco.editor.defineTheme('lodestone-dark', {
        base: 'vs-dark',
        inherit: true,
        rules: [],
        colors: {
          'editor.background': '#26282C',
          'editor.lineHighlightBackground': '#2c2e33',
        },
      });
    }
  }, [monaco]);

  /* Helper functions */

  const saveFile = async () => {
    if (!targetFile) throw new Error('No file selected');
    const error = await catchAsyncToString(
      axiosWrapper<null>({
        method: 'put',
        url: `/instance/${instance.uuid}/fs/write/${targetFile.path}`,
        data: edittedFileContent,
      })
    );
    if (error) {
      // TODO: better error display
      alert(error);
      return;
    }
    queryClient.setQueriesData(
      ['instance', instance.uuid, 'fileContent', targetFile.path],
      edittedFileContent
    );

    if (fileList) {
      const newFileList = fileList.map((file) => {
        if (file.path === targetFile.path) {
          return {
            ...file,
            modification_time: Math.round(Date.now() / 1000),
          };
        }
        return file;
      });

      queryClient.setQueriesData(
        ['instance', instance.uuid, 'fileList', path],
        newFileList
      );
    }
  };

  const deleteFile = async () => {
    if (!targetFile) throw new Error('No file selected');
    const error = await catchAsyncToString(
      axiosWrapper<null>({
        method: 'delete',
        url: `/instance/${instance.uuid}/fs/rm/${targetFile.path}`,
      })
    );
    if (error) {
      // TODO: better error display
      alert(error);
      return;
    }
    queryClient.setQueriesData(
      ['instance', instance.uuid, 'fileList', parentPath(targetFile.path)],
      fileList?.filter((file) => file.path !== targetFile.path)
    );
    setTargetFile(null);
  };

  const deleteDirectory = async () => {
    const error = await catchAsyncToString(
      axiosWrapper<null>({
        method: 'delete',
        url: `/instance/${instance.uuid}/fs/rmdir/${path}`,
      })
    );
    if (error) {
      // TODO: better error display
      alert(error);
      return;
    }
    queryClient.setQueriesData(
      ['instance', instance.uuid, 'fileList', parentPath(path)],
      fileList?.filter((file) => file.path !== path)
    );
    setPath(parentPath);
  };

  const createFile = async (name: string) => {
    return await catchAsyncToString(
      axiosWrapper<null>({
        method: 'put',
        url: `/instance/${instance.uuid}/fs/new/${path}/${name}`,
      })
    );
  };

  const createDirectory = async (name: string) => {
    if (!name) throw new Error('No name provided');
    return await catchAsyncToString(
      axiosWrapper<null>({
        method: 'put',
        url: `/instance/${instance.uuid}/fs/mkdir/${path}/${name}`,
      })
    );
  };

  const downloadFile = async () => {
    if (!targetFile) throw new Error('No file selected');
    // first we fetch a download token
    const tokenResponse = await axiosWrapper<string>({
      method: 'get',
      url: `/instance/${instance.uuid}/fs/download/${targetFile.path}`,
    });
    console.log(tokenResponse);
    const downloadUrl = axios.defaults.baseURL + `/file/${tokenResponse}`;
    window.open(downloadUrl, '_blank');
  };

  const uploadFiles = async (file: Array<File>) => {
    // upload all files using multipart form data
    const formData = new FormData();
    file.forEach((f) => {
      formData.append('file', f);
    });
    const error = await catchAsyncToString(
      axiosWrapper<null>({
        method: 'put',
        url: `/instance/${instance.uuid}/fs/upload/${path}`,
        data: formData,
        headers: {
          'Content-Type': 'multipart/form-data',
        },
        timeout: 0,
        onUploadProgress: (progressEvent) => {
          console.log(progressEvent);
        },
      })
    );
    if (error) {
      // TODO: better error display
      alert(error);
      return;
    }
    queryClient.invalidateQueries([
      'instance',
      instance.uuid,
      'fileList',
      path,
    ]);
  };

  const chooseFilesToUpload = async () => {
    const files = await chooseFiles();
    if (!files) return;
    // convert FileList to Array
    const fileArray = Array.from(files);
    await uploadFiles(fileArray);
  };

  const chooseFiles = async () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.click();
    return new Promise<FileList | null>((resolve) => {
      input.onchange = () => {
        resolve(input.files);
      };
    });
  };

  /* UI */

  const breadcrumb = (
    <div className="flex min-w-0 grow select-none flex-row flex-nowrap items-start gap-1 whitespace-nowrap text-base font-medium">
      <p className="truncate">
        {/* instance name */}
        <span
          className={
            path !== '' || targetFile
              ? 'cursor-pointer text-blue-accent hover:underline'
              : 'text-gray-300'
          }
          onClick={() => {
            setPath('');
            setTargetFile(null);
          }}
        >
          {instance.path.split(direcotrySeparator).pop()}
        </span>

        {/* path */}
        {path &&
          path.split(direcotrySeparator).map((p, i, arr) => {
            // display a breadcrumb, where each one when clicked goes to appropriate path
            const subPath = arr.slice(0, i + 1).join(direcotrySeparator);
            return (
              <span key={subPath}>
                <span className="text-gray-300"> {direcotrySeparator} </span>
                <span
                  className={
                    i !== arr.length - 1 || targetFile
                      ? 'cursor-pointer text-blue-accent hover:underline'
                      : 'text-gray-300'
                  }
                  onClick={() => {
                    setPath(subPath);
                    setTargetFile(null);
                  }}
                >
                  {p}
                </span>
              </span>
            );
          })}
      </p>

      {/* file name */}
      <p className="truncate text-gray-300">
        <span className="text-gray-300"> {direcotrySeparator} </span>
        {targetFile?.name}
      </p>
    </div>
  );

  const fileTreeEntryClassName =
    'flex flex-row items-center gap-4 py-2 px-4 text-base font-medium whitespace-nowrap';

  const fileTreeEntry = (file: ClientFile) => (
    <div
      key={file.path}
      className={clsx(fileTreeEntryClassName, 'hover:bg-gray-700', {
        'bg-gray-700': targetFile?.path === file.path,
        'bg-gray-800': targetFile?.path !== file.path,
      })}
    >
      <FontAwesomeIcon icon={faSquare} className="text-gray-500" />
      {fileTypeToIconMap[file.file_type]}
      <p
        className="grow truncate text-gray-300 hover:cursor-pointer hover:text-blue-accent hover:underline"
        onClick={() => {
          if (file.file_type === 'Directory') {
            setPath(file.path);
            setTargetFile(null);
          } else {
            setTargetFile(file);
          }
        }}
      >
        {file.name}
      </p>

      <p className="hidden min-w-[10ch] text-right text-gray-500 @xs:inline">
        {file.modification_time || file.creation_time
          ? formatTimeAgo(
              Number(file.modification_time ?? file.creation_time) * 1000
            )
          : 'Unknown Time'}
      </p>
    </div>
  );
  const fileTree = (
    <div className="flex h-full w-full grow flex-col @container/file-tree">
      <div className="overflow-y-overlay flex h-0 grow flex-col divide-y divide-gray-faded/30 overflow-x-hidden">
        {!atTopLevel ? (
          <div
            key={'..'}
            className="group flex flex-row items-center gap-4 bg-gray-800 py-2 px-4 hover:cursor-pointer hover:bg-gray-700 hover:text-blue-accent hover:underline"
            onClick={() => {
              setPath(parentPath);
              setTargetFile(null);
            }}
          >
            <p className="select-none text-base font-medium">..</p>
          </div>
        ) : null}

        {fileListLoading ? (
          <div className={fileTreeEntryClassName}>
            <p className="text-base font-medium text-gray-400">Loading...</p>
          </div>
        ) : fileListError ? (
          <div className={fileTreeEntryClassName}>
            <p className="text-base font-medium text-gray-400">
              {fileListError.message}
            </p>
          </div>
        ) : null}

        {fileList?.length === 0 && (
          <div className={fileTreeEntryClassName}>
            <p className="text-base font-medium text-gray-400">
              No files here...
            </p>
          </div>
        )}
        {fileList?.map(fileTreeEntry)}
        <div
          onClick={() => setTargetFile(null)}
          className="min-h-[25%] grow"
        ></div>
      </div>
    </div>
  );

  return (
    <div className="flex h-full w-full flex-col gap-3">
      <div className="flex flex-row items-center justify-between gap-4">
        <Menu as="div" className="relative inline-block text-left">
          <Menu.Button
            as={Button}
            label="Add/Remove"
            iconRight={faCaretDown}
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
            <Menu.Items className="absolute -left-0.5 z-10 mt-2 w-48 origin-top-left divide-y divide-gray-faded/30 rounded-lg border border-gray-faded/30 bg-gray-800 drop-shadow-md">
              {/* <div className="p-1">
                <Menu.Item>
                  {({ active }) => (
                    <Formik
                      initialValues={{ name: '' }}
                      onSubmit={async (
                        values: { name: string },
                        actions: any
                      ) => {
                        actions.setSubmitting(true);
                        const error = await createFile(values.name);
                        if (error) {
                          alert(error);
                          actions.setErrors({ name: error });
                          actions.setSubmitting(false);
                        } else {
                          queryClient.setQueriesData(
                            ['instance', instance.uuid, 'fileList', path],
                            fileList
                              ? [
                                  ...fileList,
                                  {
                                    name: values.name,
                                    path: `${path}/${values.name}`,
                                    file_type: 'File' as FileType,
                                    creation_time: Date.now() / 1000,
                                    modification_time: Date.now() / 1000,
                                  },
                                ].sort(fileSorter)
                              : undefined
                          );
                          actions.setSubmitting(false);
                          actions.resetForm();
                        }
                      }}
                    >
                      <Form id="create-file-form" autoComplete="off">
                        <InputField name="name" placeholder="New File" />
                      </Form>
                    </Formik>
                  )}
                </Menu.Item>
                <Menu.Item>
                  {({ active }) => (
                    <Formik
                      initialValues={{ name: '' }}
                      onSubmit={async (
                        values: { name: string },
                        actions: any
                      ) => {
                        actions.setSubmitting(true);
                        const error = await createDirectory(values.name);
                        if (error) {
                          alert(error);
                          actions.setErrors({ name: error });
                          actions.setSubmitting(false);
                        } else {
                          queryClient.setQueriesData(
                            ['instance', instance.uuid, 'fileList', path],
                            fileList
                              ? [
                                  ...fileList,
                                  {
                                    name: values.name,
                                    path: `${path}/${values.name}`,
                                    file_type: 'Directory' as FileType,
                                    creation_time: Date.now() / 1000,
                                    modification_time: Date.now() / 1000,
                                  },
                                ].sort(fileSorter)
                              : undefined
                          );
                          actions.setSubmitting(false);
                          actions.resetForm();
                        }
                      }}
                    >
                      <Form id="create-directory-form" autoComplete="off">
                        <InputField name="name" placeholder="New Folder" />
                      </Form>
                    </Formik>
                  )}
                </Menu.Item>
              </div> */}
              <div className="p-2">
                <Menu.Item>
                  {({ active, disabled }) => (
                    <Button
                      label="Delete directory"
                      className="w-full items-start whitespace-nowrap py-1.5 font-normal"
                      onClick={deleteDirectory}
                      icon={faTrashCan}
                      variant="text"
                      align="start"
                      color="red"
                      disabled={disabled}
                      active={active}
                    />
                  )}
                </Menu.Item>
                <Menu.Item disabled={isFileLoading || !targetFile}>
                  {({ active, disabled }) => (
                    <Button
                      className="w-full whitespace-nowrap py-1.5 font-normal"
                      label={`Delete file`}
                      icon={faTrashCan}
                      onClick={() => deleteFile()}
                      variant="text"
                      align="start"
                      color="red"
                      disabled={disabled}
                      active={active}
                    />
                  )}
                </Menu.Item>
              </div>
            </Menu.Items>
          </Transition>
        </Menu>

        {breadcrumb}
        <Button
          className="h-fit"
          label="Save"
          icon={faFloppyDisk}
          onClick={saveFile}
          disabled={
            !targetFile ||
            edittedFileContent === originalFileContent ||
            !showingMonaco
          }
        />
        <Button
          className="h-fit"
          label="Upload"
          icon={faUpload}
          onClick={chooseFilesToUpload}
        />
        <Button
          className="h-fit"
          label="Download"
          icon={faDownload}
          onClick={downloadFile}
          disabled={!targetFile}
        />
      </div>
      <div className="flex h-full w-full flex-row divide-x divide-gray-faded/30 overflow-clip rounded-lg border border-gray-faded/30 bg-gray-800">
        <ResizePanel
          direction="e"
          maxSize={500}
          minSize={200}
          size={fileListSize}
          validateSize={false}
          onResize={setFileListSize}
          containerClassNames="grow"
          grow={!targetFile}
        >
          {fileTree}
        </ResizePanel>
        {targetFile && (
          <div className="min-w-0 grow">
            <div className="h-full">
              {showingMonaco ? (
                <Editor
                  height="100%"
                  onChange={(value) => {
                    setEdittedFileContent(value ?? '');
                  }}
                  value={edittedFileContent}
                  theme="lodestone-dark"
                  path={monacoPath}
                  className="overflow-clip bg-gray-800"
                  options={{
                    padding: {
                      top: 8,
                    },
                    minimap: {
                      enabled: false,
                    },
                    lineNumbersMinChars: 3,
                  }}
                  onMount={handleEditorDidMount}
                />
              ) : (
                <div className="flex h-full w-full flex-col items-center justify-center gap-4 bg-gray-800">
                  <FontAwesomeIcon
                    icon={faFilePen}
                    className="text-xlarge text-gray-500"
                  />
                  <p className="text-xl text-center text-gray-400">
                    File Editor
                  </p>
                  <p className="text-xl text-center text-gray-400">
                    {fileError
                      ? fileError?.message ?? 'Unknown Error'
                      : isFileLoading
                      ? 'Loading...'
                      : 'Select a file to view its contents'}
                  </p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
