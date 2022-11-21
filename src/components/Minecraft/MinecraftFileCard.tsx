import Editor, { DiffEditor, useMonaco, loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faFolder,
  faFile,
  faClipboardQuestion,
  IconDefinition,
} from '@fortawesome/free-solid-svg-icons';
import { useContext, useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { File } from 'bindings/File';
import { InstanceContext } from 'data/InstanceContext';
import axios, { AxiosError } from 'axios';
import { FileType } from 'bindings/FileType';
import { axiosWrapper, catchAsyncToString, formatTimeAgo } from 'utils/util';
import Button from 'components/Atoms/Button';
import { ClientError } from 'bindings/ClientError';
import { useDebounce, useEffectOnce, useLocalStorage } from 'usehooks-ts';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik } from 'formik';
import ResizePanel from 'components/Atoms/ResizePanel';

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

export default function MinecraftFileCard() {
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
  if (instance.path.includes('/'))
    direcotrySeparator = '/';

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
    ['instance', instance.uuid, 'files', path],
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
    ['instance', instance.uuid, 'files', path, targetFile?.path],
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
      ['instance', instance.uuid, 'files', path, targetFile.path],
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
      console.log(fileList, newFileList);

      queryClient.setQueriesData(
        ['instance', instance.uuid, 'files', path],
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
      [
        'instance',
        instance.uuid,
        'files',
        parentPath(targetFile.path),
      ],
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
      [
        'instance',
        instance.uuid,
        'files',
        parentPath(path),
      ],
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
    return await catchAsyncToString(
      axiosWrapper<null>({
        method: 'put',
        url: `/instance/${instance.uuid}/fs/mkdir/${path}/${name}`,
      })
    );
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

  const breadcrumb = (
    <p className="select-none px-2 py-2 text-medium font-medium">
      <span>
        <span
          className={
            path !== ''
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
        <span className="text-gray-300"> / </span>
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
            {i !== arr.length - 1 && <span className="text-gray-300"> {direcotrySeparator} </span>}
          </span>
        );
      })}
    </p>
  );

  return (
    <div className="flex h-full w-full flex-col gap-2">
      {breadcrumb}
      <div className="flex h-full w-full flex-row">
        <ResizePanel
          direction="e"
          maxSize={500}
          minSize={200}
          size={fileListSize}
          validateSize={false}
          onResize={setFileListSize}
          resizeBarClassNames="cursor-ew-resize pl-4 pr-4"
        >
          <div className="flex h-full w-1/4 grow flex-col">
            <div className="flex h-0 grow flex-col gap-2 overflow-y-auto overflow-x-hidden">
              <div className="overflow-x-hidden rounded-lg border border-gray-faded/30">
                {!atTopLevel ? (
                  <div
                    key={'..'}
                    className="group flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4 last:border-b-0 hover:cursor-pointer hover:bg-gray-700 hover:text-blue-accent hover:underline"
                    onClick={() => {
                      setPath(parentPath);
                      setTargetFile(null);
                    }}
                  >
                    <p className="select-none text-base font-medium">..</p>
                  </div>
                ) : null}

                {fileListLoading ? (
                  <div className="flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4 last:border-b-0">
                    <p className="text-base font-medium text-gray-400">
                      Loading...
                    </p>
                  </div>
                ) : fileListError ? (
                  <div className="flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4 last:border-b-0">
                    <p className="text-base font-medium text-gray-400">
                      {fileListError.message}
                    </p>
                  </div>
                ) : null}

                {fileList?.length === 0 && (
                  <div className="flex flex-row items-center gap-4 border-b border-gray-faded/30 bg-gray-800 py-2 px-4 last:border-b-0">
                    <p className="text-base font-medium text-gray-400">
                      No files here...
                    </p>
                  </div>
                )}

                {fileList?.map((file) => (
                  <div
                    key={file.path}
                    className={`flex flex-row items-center gap-4 border-b border-gray-faded/30 py-2 px-4 last:border-b-0 hover:bg-gray-700 ${
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
                            Number(
                              file.modification_time ?? file.creation_time
                            ) * 1000
                          )
                        : 'Unknown Time'}
                    </p>
                  </div>
                ))}
              </div>
            </div>
            <div className="flex h-[10%] flex-row items-center justify-between gap-4 border-b border-gray-faded/30 last:border-b-0">
              <Formik
                initialValues={{ name: '' }}
                onSubmit={async (values: { name: string }, actions: any) => {
                  actions.setSubmitting(true);
                  const error = await createFile(values.name);
                  if (error) {
                    alert(error);
                    actions.setErrors({ name: error });
                    actions.setSubmitting(false);
                  } else {
                    queryClient.setQueriesData(
                      ['instance', instance.uuid, 'files', path],
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
              <Formik
                initialValues={{ name: '' }}
                onSubmit={async (values: { name: string }, actions: any) => {
                  actions.setSubmitting(true);
                  const error = await createDirectory(values.name);
                  if (error) {
                    alert(error);
                    actions.setErrors({ name: error });
                    actions.setSubmitting(false);
                  } else {
                    queryClient.setQueriesData(
                      ['instance', instance.uuid, 'files', path],
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
              <Button
                label="rm -r ."
                className="whitespace-nowrap"
                onClick={deleteDirectory}
              />
            </div>
          </div>
        </ResizePanel>
        <div className="min-w-0 grow">
          <div className="h-full">
            {showingMonaco ? (
              <Editor
                height="90%"
                onChange={(value) => {
                  setEdittedFileContent(value ?? '');
                }}
                value={edittedFileContent}
                theme="lodestone-dark"
                path={monacoPath}
                className="overflow-clip rounded-lg border border-gray-faded/30 bg-gray-800"
                options={{
                  padding: {
                    top: 8,
                  },
                }}
                onMount={handleEditorDidMount}
              />
            ) : (
              <div className="flex h-[90%] w-full flex-col items-center justify-center rounded-lg bg-gray-800">
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
            {targetFile && (
              <div className="flex h-[10%] flex-row items-center gap-2 child:my-2">
                <Button
                  className="h-fit"
                  label={`Save ${targetFile.name}`}
                  onClick={() => saveFile()}
                  disabled={
                    edittedFileContent === originalFileContent || !showingMonaco
                  }
                />
                <div className="grow"></div>
                <Button
                  className="h-fit text-red"
                  label={`Delete ${targetFile.name}`}
                  onClick={() => deleteFile()}
                  disabled={isFileLoading}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
