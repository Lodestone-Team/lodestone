import {
  faClipboardQuestion,
  faFile,
  faFilePen,
  faFolder,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ClientFile } from 'bindings/ClientFile';
import clsx from 'clsx';
import Checkbox from 'components/Atoms/Checkbox';
import { formatTimeAgo } from 'utils/util';

export default function FileList({
  path,
  fileList,
  loading,
  error,
  tickedFiles,
  tickFile,
  openedFile,
  atTopLevel,
  onParentClick,
  onEmptyClick,
  onFileClick,
}: {
  path: string;
  fileList: ClientFile[] | undefined;
  loading: boolean;
  error: Error | null;
  tickedFiles: ClientFile[];
  tickFile: (file: ClientFile, ticked: boolean) => void;
  openedFile: ClientFile | null;
  atTopLevel: boolean;
  onParentClick: () => void;
  onEmptyClick: () => void;
  onFileClick: (file: ClientFile) => void;
}) {
  const fileTicked = (file: ClientFile) => {
    // check just the path and type, not other metadata
    return tickedFiles.some(
      (f) => f.path === file.path && f.file_type === file.file_type
    );
  };

  const fileTreeEntryClassName =
    'flex flex-row items-center gap-4 py-2 px-4 text-medium font-medium tracking-medium whitespace-nowrap';

  return (
    <div className="flex h-full w-full grow flex-col @container/file-tree">
      <div className="overflow-y-overlay flex h-0 grow flex-col divide-y divide-gray-faded/30 overflow-x-hidden">
        {!atTopLevel ? (
          <div
            key={'..'}
            className="group flex flex-row items-center gap-4 bg-gray-800 py-2 px-4 hover:cursor-pointer hover:bg-gray-700 hover:text-blue-200 hover:underline"
            onClick={onParentClick}
          >
            <p className="select-none text-medium font-medium">..</p>
          </div>
        ) : null}

        {loading ? (
          <div className={fileTreeEntryClassName}>
            <p className="text-medium font-medium text-gray-400">Loading...</p>
          </div>
        ) : error ? (
          <div className={fileTreeEntryClassName}>
            <p className="text-medium font-medium text-gray-400">
              {error.message}
            </p>
          </div>
        ) : null}

        {fileList?.length === 0 && (
          <div className={fileTreeEntryClassName}>
            <p className="text-medium font-medium text-gray-400">
              No files here...
            </p>
          </div>
        )}

        {fileList?.map((file: ClientFile) => (
          <div
            key={file.path}
            className={clsx(fileTreeEntryClassName, 'hover:bg-gray-700', {
              'bg-gray-700': fileTicked(file),
              'bg-gray-800': !fileTicked(file),
            })}
          >
            <Checkbox
              checked={fileTicked(file)}
              onChange={(ticked) => {
                tickFile(file, ticked);
              }}
            />
            <div className="w-3">
              {file.file_type === 'Directory' && (
                <FontAwesomeIcon icon={faFolder} className="text-blue-200" />
              )}
              {file.file_type === 'File' && (
                <FontAwesomeIcon
                  icon={openedFile?.path === file.path ? faFilePen : faFile}
                  className="text-gray-400"
                />
              )}
              {file.file_type === 'Unknown' && (
                <FontAwesomeIcon
                  icon={faClipboardQuestion}
                  className="text-yellow"
                />
              )}
            </div>
            <p
              className={clsx(
                'truncate text-gray-300 hover:cursor-pointer hover:text-blue-200 hover:underline',
                openedFile?.path === file.path && 'italic'
              )}
              onClick={() => onFileClick(file)}
            >
              {file.name}
            </p>
            <div className="grow"></div>

            <p className="hidden min-w-[10ch] text-right text-gray-500 @xs:inline">
              {file.modification_time || file.creation_time
                ? formatTimeAgo(
                    Number(file.modification_time ?? file.creation_time) * 1000
                  )
                : 'Unknown Time'}
            </p>
          </div>
        ))}
        <div onClick={onEmptyClick} className="min-h-[25%] grow"></div>
      </div>
    </div>
  );
}
