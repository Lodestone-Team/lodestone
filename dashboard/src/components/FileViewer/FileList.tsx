import { faClipboardQuestion } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ClientFile } from 'bindings/ClientFile';
import clsx from 'clsx';
import Checkbox from 'components/Atoms/Checkbox';
import { formatBytes, formatTimeAgo } from 'utils/util';
import FileContextMenu from './FileContextMenu';
import React, {
  RefObject,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useOnClickOutside } from 'usehooks-ts';
import { useFileViewer } from './FileViewerContext';
import { SkeletonGroup, SkeletonRows } from 'components/Atoms/Skeleton';
import { useT } from 'i18n';
import { asset } from 'utils/basePath';

// a map from file extension to icon
const iconMap: { [key: string]: string } = {
  txt: '/icons/document.svg',
  md: '/icons/document.svg',
  properties: '/icons/document.svg',
  yml: '/icons/yaml.svg',
  yaml: '/icons/yaml.svg',
  exe: '/icons/exe.svg',
  cfg: '/icons/document.svg',
  conf: '/icons/document.svg',
  config: '/icons/document.svg',
  ini: '/icons/document.svg',
  png: '/icons/image.svg',
  jpg: '/icons/image.svg',
  jpeg: '/icons/image.svg',
  gif: '/icons/image.svg',
  bmp: '/icons/image.svg',
  svg: '/icons/image.svg',
  json: '/icons/json.svg',
  mcmeta: '/icons/json.svg',
  zip: '/icons/zip.svg',
  gz: '/icons/zip.svg',
  tar: '/icons/zip.svg',
  jar: '/icons/jar.svg',
  java: '/icons/java.svg',
  class: '/icons/javaclass.svg',
  ts: '/icons/typescript.svg',
  js: '/icons/javascript.svg',
  readme: '/icons/readme.svg',
  bin: '/icons/binary.svg',
  dll: '/icons/binary.svg',
  dylib: '/icons/binary.svg',
  so: '/icons/binary.svg',
  o: '/icons/binary.svg',
  dat: '/icons/3d.svg',
  dat_old: '/icons/3d.svg',
  obj: '/icons/3d.svg',
  nbt: '/icons/3d.svg',
  mca: '/icons/3d.svg',
  lock: '/icons/lock.svg',
  log: '/icons/log.svg',
  ps1: '/icons/pwsh.svg',
  sh: '/icons/console.svg',
  mcfunction: '/icons/minecraft.svg',
};

export default function FileList({
  fileList,
  loading,
  stale = false,
  error,
  atTopLevel,
  onParentClick,
  onEmptyClick,
  onFileClick,
}: {
  fileList: ClientFile[] | undefined;
  loading: boolean;
  /** fileList is the previous folder's, kept on screen while this one loads */
  stale?: boolean;
  error: Error | null;
  atTopLevel: boolean;
  onParentClick: () => void;
  onEmptyClick: () => void;
  onFileClick: (file: ClientFile) => void;
}) {
  const t = useT();
  const { tickedFiles, tickFile, openedFile, clipboard, clipboardAction } =
    useFileViewer();

  // check just the path and type, not other metadata
  const tickedKeys = useMemo(
    () => new Set(tickedFiles.map((f) => `${f.file_type}:${f.path}`)),
    [tickedFiles]
  );
  const fileTicked = (file: ClientFile) =>
    tickedKeys.has(`${file.file_type}:${file.path}`);

  const boundingDivRef = useRef<HTMLDivElement>(null);
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const [showContextMenu, setShowContextMenu] = useState(false);
  // where the menu was opened, relative to boundingDivRef
  const [contextMenuPointer, setContextMenuPointer] = useState({ x: 0, y: 0 });
  const [contextMenuCoords, setContextMenuCoords] = useState({ x: 0, y: 0 });
  const [contextMenuFile, setContextMenuFile] = useState<ClientFile | null>();

  useOnClickOutside(contextMenuRef as RefObject<HTMLDivElement>, () =>
    setShowContextMenu(false)
  );

  function fileToIcon(file: ClientFile) {
    if (file.file_type === 'Directory') {
      return (
        <img
          src="/icons/folder-blue.svg"
          alt={t('files.list.folderIcon')}
          draggable="false"
        />
      );
    } else if (file.file_type === 'File') {
      if (file.file_stem === '.lodestone_config') {
        return (
          <img
            src="/icons/lodestone.svg"
            alt={t('files.list.fileIcon')}
            draggable="false"
            // because the lodestone icon is a bit bigger than the others
            // set between 4 and 5
            className="h-4 w-4"
          />
        );
      }
      const fileExt = file.extension ?? '';
      if (fileExt in iconMap) {
        return (
          <img
            src={asset(iconMap[fileExt])}
            alt={t('files.list.fileIcon')}
            draggable="false"
            className="h-4 w-4"
          />
        );
      } else {
        return (
          <img
            src="/icons/file.svg"
            alt={t('files.list.fileIcon')}
            draggable="false"
            className="h-4 w-4"
          />
        );
      }
    }

    return (
      <FontAwesomeIcon icon={faClipboardQuestion} className="text-yellow" />
    );
  }

  // remember where the menu was opened; it is placed once it has rendered
  const openContextMenu = (e: React.MouseEvent, file: ClientFile | null) => {
    e.preventDefault();
    const boundingDiv = boundingDivRef.current;
    if (!boundingDiv) return;
    const rect = boundingDiv.getBoundingClientRect();
    setContextMenuPointer({
      x: e.clientX - (rect.left + window.scrollX),
      y: e.clientY - (rect.top + window.scrollY),
    });
    setContextMenuFile(file);
    setShowContextMenu(true);
  };

  // place the menu at the pointer, kept inside the list. Its size is measured
  // (before it is painted): it depends on the items and the language.
  useLayoutEffect(() => {
    const boundingDiv = boundingDivRef.current;
    const menu = contextMenuRef.current;
    if (!showContextMenu || !boundingDiv || !menu) return;
    const mousePos = contextMenuPointer;
    const boundingDivDimensions = {
      height: boundingDiv.offsetHeight,
      width: boundingDiv.offsetWidth,
    };
    // the menu's top margin is part of its height here
    const width = menu.offsetWidth;
    const height = menu.offsetHeight + 6;
    let x = null;
    let y = null;

    if (mousePos.x + width > boundingDivDimensions.width) {
      x = mousePos.x - width;
    } else {
      x = mousePos.x;
    }
    if (mousePos.y + height > boundingDivDimensions.height - 10) {
      y = boundingDivDimensions.height - height - 10;
    } else {
      y = mousePos.y;
    }
    if (
      mousePos.x + width > boundingDivDimensions.width &&
      mousePos.x - width < 4
    ) {
      x = 4;
    }

    setContextMenuCoords({ x, y });
  }, [showContextMenu, contextMenuPointer, contextMenuFile]);

  const fileTreeEntryClassName =
    'flex flex-row items-center gap-4 py-2 px-4 text-medium font-medium tracking-medium whitespace-nowrap';

  return (
    <div
      className="@container/file-tree flex h-full w-full grow flex-col"
      ref={boundingDivRef}
    >
      {showContextMenu && (
        <FileContextMenu
          ref={contextMenuRef}
          file={contextMenuFile ?? null}
          coords={contextMenuCoords}
          setShowContextMenu={setShowContextMenu}
        />
      )}
      <div className="flex h-0 grow flex-col divide-y divide-gray-faded/30 overflow-x-hidden overflow-y-overlay">
        {!atTopLevel ? (
          <div
            key={'..'}
            className="group flex flex-row items-center gap-4 bg-gray-800 px-4 py-2 hover:cursor-pointer hover:bg-gray-700 hover:text-blue-200 hover:underline"
            onClick={onParentClick}
          >
            <p className="text-medium font-medium select-none">..</p>
          </div>
        ) : null}

        {loading ? (
          // file rows (checkbox, icon, name, size); they appear after a
          // moment, so a folder that opens quickly doesn't flash them
          <SkeletonGroup label={t('files.list.loading')}>
            <SkeletonRows
              rows={8}
              icon="square"
              trailing
              className="divide-y divide-gray-faded/30"
              rowClassName={clsx(fileTreeEntryClassName, 'h-10 bg-gray-800')}
            />
          </SkeletonGroup>
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
              {t('files.list.empty')}
            </p>
          </div>
        )}
        {fileList?.map((file: ClientFile) => {
          // compare paths: the objects are replaced on every refetch
          const isOpened = openedFile?.path === file.path;
          const isTicked = fileTicked(file);
          return (
            <div
              key={file.path}
              className={clsx(fileTreeEntryClassName, 'hover:bg-gray-700', {
                'bg-gray-900': isOpened,
                'bg-gray-700': isTicked && !isOpened,
                'bg-gray-800': !isTicked && !isOpened,
                // the previous folder, until the new one has loaded
                'pointer-events-none opacity-50 transition-opacity delay-150':
                  stale,
              })}
              onContextMenu={(e) => openContextMenu(e, file)}
            >
              <Checkbox
                checked={isTicked}
                onChange={(ticked) => {
                  tickFile(file, ticked);
                }}
              />
              <div className="h-4 w-4 shrink-0">{fileToIcon(file)}</div>
              <p
                className={clsx(
                  'truncate hover:cursor-pointer hover:underline',
                  isOpened && 'italic',
                  clipboardAction === 'cut' &&
                    clipboard?.some((f) => f.path === file.path)
                    ? 'text-gray-300/60 hover:text-blue-200/60'
                    : 'text-gray-300 hover:text-blue-200'
                )}
                onClick={() => onFileClick(file)}
              >
                {file.name}
              </p>
              <div className="grow"></div>

              <p className="hidden min-w-[8ch] text-right text-gray-500 @xs:inline">
                {file.modification_time || file.creation_time
                  ? formatTimeAgo(
                      Number(file.modification_time ?? file.creation_time) *
                        1000
                    )
                  : t('files.list.unknownTime')}
              </p>

              <p className="hidden min-w-[8ch] text-right text-gray-500 @sm:inline">
                {file.file_type === 'Directory'
                  ? ''
                  : file.size
                    ? formatBytes(file.size, 1)
                    : ''}
              </p>
            </div>
          );
        })}
        <div
          onClick={onEmptyClick}
          className="min-h-[25%] grow"
          onContextMenu={(e) => openContextMenu(e, null)}
        ></div>
      </div>
    </div>
  );
}
