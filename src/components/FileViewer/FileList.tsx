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
import FileContextMenu from './FileContextMenu';
import React, { useState, useEffect, useRef, } from 'react';
import { useFileContent, useFileList } from 'data/FileSystem';

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
  setCreateFolderModalOpen,
  setCreateFileModalOpen,
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
  setCreateFileModalOpen: (modalOpen: boolean) => void;
  setCreateFolderModalOpen: (modalOpen: boolean) => void;
}) {
  const fileTicked = (file: ClientFile) => {
    // check just the path and type, not other metadata
    return tickedFiles.some(
      (f) => f.path === file.path && f.file_type === file.file_type
    );
  };

  const boundingDivRef = useRef(null) 
  const contextMenuRef = useRef(null) 
  const [mousePos, setMousePos] = useState({});
  const [showContextMenu, setShowContextMenu] = useState(false);
  const [contextMenuCoords, setContextMenuCoords] = useState({x: 0, y: 0});
  const [contextMenuFile, setContextMenuFile] = useState({});
  const [absCoords, setAbsCoords] = useState({x: 0, y: 0})
  const [contextMenuDimensions, setContextMenuDimensions] = useState({ height: 0, width: 0})
  const [boundingDivDimensions, setBoundingDivDimensions] = useState({ height: 0, width: 0})

  useEffect(() => {
    if (contextMenuRef.current !== null) {
      setContextMenuDimensions({ height: contextMenuRef.current.offsetHeight, width: contextMenuRef.current.offsetWidth })
    }
  }, [contextMenuRef]);

  useEffect(() => {
    if (boundingDivRef.current !== null) {
      setBoundingDivDimensions({ height: boundingDivRef.current.offsetHeight, width: boundingDivRef.current.offsetWidth })
    }
  }, [boundingDivRef]);

  useEffect(() => {
    const handleMouseMove = (e) => {
      setMousePos({ x: e.clientX - absCoords.x, y: e.clientY - absCoords.y });
    };

    window.addEventListener('mousemove', handleMouseMove);

    return () => {
      window.removeEventListener(
        'mousemove',
        handleMouseMove
      );
    };
  }, [absCoords]);

  useEffect(() => {
    const onResize = () => {
      setAbsCoords({x: boundingDivRef.current.getBoundingClientRect().left + window.scrollX, y: boundingDivRef.current.getBoundingClientRect().top + window.scrollY });
      if (contextMenuRef.current !== null) {
        setContextMenuDimensions({ height: contextMenuRef.current.offsetHeight, width: contextMenuRef.current.offsetWidth })
      }
      if (boundingDivRef.current !== null) {
        setBoundingDivDimensions({ height: boundingDivRef.current.offsetHeight, width: boundingDivRef.current.offsetWidth })
      }
    }

    setAbsCoords({x: boundingDivRef.current.getBoundingClientRect().left + window.scrollX, y: boundingDivRef.current.getBoundingClientRect().top + window.scrollY });

    window.addEventListener('resize', onResize);
    return () => {
        window.removeEventListener('resize', onResize);
    };

  }, [])

  useEffect(() => {
    function handleClickOutside(event) {
      if (contextMenuRef.current && !contextMenuRef.current.contains(event.target)) {
        setShowContextMenu(false)
      }
    }
    window.addEventListener("mousedown", handleClickOutside);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);
  

  const calculateStartMenuCoords = () => {
    let x = null;
    let y = null;
    if (mousePos.x + contextMenuDimensions.width > boundingDivDimensions.width) {
      x = mousePos.x - contextMenuDimensions.width;
      console.log(x)
    } else {
      x = mousePos.x
    }
    console.log(boundingDivDimensions.height)
    if (mousePos.y + contextMenuDimensions.height > boundingDivDimensions.height) {
      y = boundingDivDimensions.height - contextMenuDimensions.height - 10
      console.log(y)
    } else {
      y = mousePos.y
    }
    setContextMenuCoords({ x, y })
    
  }

  const fileTreeEntryClassName =
    'flex flex-row items-center gap-4 py-2 px-4 text-medium font-medium tracking-medium whitespace-nowrap';

  return (
    <div className="flex h-full w-full grow flex-col @container/file-tree" ref={boundingDivRef}>
      { showContextMenu && 
        <FileContextMenu 
          ref={contextMenuRef} 
          file={""} 
          coords={contextMenuCoords} 
          setCreateFileModalOpen={setCreateFileModalOpen} 
          setCreateFolderModalOpen={setCreateFolderModalOpen} 
        /> 
      }
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
            onContextMenu={(e) => { e.preventDefault(); 
              console.log("The gamer said one thing and one thing only... \"it's gamer time\".`");
              console.log(mousePos)
              setContextMenuFile(file)
              calculateStartMenuCoords()
              setShowContextMenu(true)
            }}
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
