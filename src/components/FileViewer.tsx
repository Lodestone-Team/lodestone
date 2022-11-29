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
} from '@fortawesome/free-solid-svg-icons';
import { Fragment, useContext, useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { File } from 'bindings/File';
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

type Monaco = typeof monaco;

const fileTypeToIconMap: Record<FileType, React.ReactElement> = {
  Directory: <FontAwesomeIcon icon={faFolder} className="text-blue-accent" />,
  File: <FontAwesomeIcon icon={faFile} className="text-gray-400" />,
  Unknown: (
    <FontAwesomeIcon icon={faClipboardQuestion} className="text-ochre" />
  ),
};

const fileSorter = (a: File, b: File) => {
  if (a.file_type === b.file_type) {
    return a.name.localeCompare(b.name);
  }
  return a.file_type.localeCompare(b.file_type);
};

export default function FileViewer() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  const monaco = useMonaco();
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  if (!instance) throw new Error('No instance selected');
  const [path, setPath] = useState('');
  const [targetFile, setTargetFile] = useState<File | null>(null);
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

  const {
    data: fileList,
    isLoading: fileListLoading,
    error: fileListError,
  } = useQuery<File[], Error>(
    ['instance', instance.uuid, 'fileList', path],
    () => {
      return axiosWrapper<File[]>({
        url: `/instance/${instance.uuid}/fs/ls/${path}`,
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

  const {
    data: originalFileContent,
    isLoading: isFileLoading,
    error: fileError,
  } = useQuery<string, Error>(
    ['instance', instance.uuid, 'fileContent', targetFile?.path],
    () => {
      setEdittedFileContent('');
      return axiosWrapper<string>({
        url: `/instance/${instance.uuid}/fs/read/${targetFile?.path}`,
        method: 'GET',
        transformResponse: (data) => data,
      }).then((response) => {
        setEdittedFileContent(response);
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
    // TODO: new temporary download link
    const downloadUrl =
      axios.defaults.baseURL +
      `/instance/${instance.uuid}/fs/download/${targetFile.path}`;
    window.open(downloadUrl, '_blank');
  };

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

  console.log('original content', originalFileContent);
  console.log('editted content', edittedFileContent);

  const breadcrumb = (
    <p className="grow select-none text-base font-medium">
      <span>
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
        <span className="text-gray-300"> {direcotrySeparator} </span>
      </span>

      {path.split(direcotrySeparator).map((p, i, arr) => {
        // display a breadcrumb, where each one when clicked goes to appropriate path
        const subPath = arr.slice(0, i + 1).join(direcotrySeparator);
        return (
          <span key={subPath}>
            <span
              className={
                i !== arr.length - 1
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
            {i !== arr.length - 1 && (
              <span className="text-gray-300"> {direcotrySeparator} </span>
            )}
          </span>
        );
      })}
      <span>
        <span className="text-gray-300">{targetFile?.name}</span>
      </span>
    </p>
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
            <Menu.Items className="absolute left-0 z-10 mt-2 w-48 origin-top-left divide-y divide-gray-faded/30 rounded-lg border border-gray-faded/30 bg-gray-700 drop-shadow-md">
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
          label="Download"
          icon={faDownload}
          onClick={downloadFile}
          disabled={!targetFile}
        />
      </div>
      <div className="flex h-full w-full flex-row overflow-clip rounded-lg border border-gray-faded/30 bg-gray-800">
        <ResizePanel
          direction="e"
          maxSize={500}
          minSize={200}
          size={fileListSize}
          validateSize={false}
          onResize={setFileListSize}
          containerClassNames="border-r border-gray-faded/30"
        >
          <div className="flex h-full w-full grow flex-col">
            <div className="flex h-0 grow flex-col overflow-y-auto overflow-x-hidden">
              {!atTopLevel ? (
                <div
                  key={'..'}
                  className="group flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4 hover:cursor-pointer hover:bg-gray-700 hover:text-blue-accent hover:underline"
                  onClick={() => {
                    setPath(parentPath);
                    setTargetFile(null);
                  }}
                >
                  <p className="select-none text-base font-medium">..</p>
                </div>
              ) : null}

              {fileListLoading ? (
                <div className="flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4">
                  <p className="text-base font-medium text-gray-400">
                    Loading...
                  </p>
                </div>
              ) : fileListError ? (
                <div className="flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4">
                  <p className="text-base font-medium text-gray-400">
                    {fileListError.message}
                  </p>
                </div>
              ) : null}

              {fileList?.length === 0 && (
                <div className="flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4">
                  <p className="text-base font-medium text-gray-400">
                    No files here...
                  </p>
                </div>
              )}

              {fileList?.map((file) => (
                <div
                  key={file.path}
                  className={`flex flex-row items-center gap-4 border-b border-gray-faded/30 py-2 px-4 hover:bg-gray-700 ${
                    file.name === targetFile?.name
                      ? 'bg-gray-750'
                      : 'bg-gray-800'
                  }`}
                >
                  {fileTypeToIconMap[file.file_type]}
                  <p
                    className="truncate text-base font-medium text-gray-300 hover:cursor-pointer hover:text-blue-accent hover:underline"
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

                  <p className="grow whitespace-nowrap text-right text-base font-medium text-gray-500">
                    {file.modification_time || file.creation_time
                      ? formatTimeAgo(
                          Number(file.modification_time ?? file.creation_time) *
                            1000
                        )
                      : 'Unknown Time'}
                  </p>
                </div>
              ))}
            </div>
          </div>
        </ResizePanel>
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
              <div className="flex h-full w-full flex-col items-center justify-center bg-gray-800">
                <FontAwesomeIcon
                  icon={faFile}
                  className="text-5xl text-gray-500"
                />
                <p className="text-xl mt-4 text-gray-400">
                  {!targetFile
                    ? 'Select a file to view its contents'
                    : fileError
                    ? fileError?.message ?? 'Unknown Error'
                    : isFileLoading
                    ? 'Loading...'
                    : 'Select a file to view its contents'}
                </p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
