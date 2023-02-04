export default function FileTree() {
    
  return (
    <div className="flex h-full w-full grow flex-col @container/file-tree">
      <div className="overflow-y-overlay flex h-0 grow flex-col divide-y divide-gray-faded/30 overflow-x-hidden">
        {!atTopLevel ? (
          <div
            key={'..'}
            className="group flex flex-row items-center gap-4 bg-gray-800 py-2 px-4 hover:cursor-pointer hover:bg-gray-700 hover:text-blue-200 hover:underline"
            onClick={() => {
              setPath(parentPath(path, direcotrySeparator), false);
            }}
          >
            <p className="select-none text-medium font-medium">..</p>
          </div>
        ) : null}

        {fileListLoading ? (
          <div className={fileTreeEntryClassName}>
            <p className="text-medium font-medium text-gray-400">Loading...</p>
          </div>
        ) : fileListError ? (
          <div className={fileTreeEntryClassName}>
            <p className="text-medium font-medium text-gray-400">
              {fileListError.message}
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

        {fileList?.map(fileTreeEntry)}
        <div
          onClick={() => {
            setOpenedFile(null);
            setTickedFiles([]);
          }}
          className="min-h-[25%] grow"
        ></div>
      </div>
    </div>
  );
}
