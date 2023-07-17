import { ClientFile } from 'bindings/ClientFile';
import { InstanceContext } from 'data/InstanceContext';
import { useContext } from 'react';

export default function Breadcrumb({
  path,
  openedFile,
  setPath,
  directorySeparator,
}: {
  path: string;
  openedFile: ClientFile | null;
  setPath: (path: string, refresh: boolean) => void;
  directorySeparator: string;
}) {
  const { selectedInstance: instance } = useContext(InstanceContext);

  if (!instance) throw new Error('No instance selected');

  return (
    <div className="flex min-w-0 grow select-none flex-row flex-nowrap items-start gap-1 truncate whitespace-nowrap text-medium font-medium">
      <p className="truncate">
        {/* instance name */}
        <span
          className={
            path !== '' || openedFile
              ? 'cursor-pointer text-blue-200 hover:underline'
              : 'text-gray-300'
          }
          onClick={() => {
            setPath('.', false);
          }}
        >
          {instance.path.split(directorySeparator).pop()}
        </span>

        {/* path */}
        {path &&
          path.split(directorySeparator).map((p, i, arr) => {
            // display a breadcrumb, where each one when clicked goes to appropriate path
            const subPath = arr.slice(0, i + 1).join(directorySeparator);
            if (subPath === '' || subPath === '.')
              return null; /* skip the first empty path */
            return (
              <span key={subPath}>
                <span className="text-gray-300"> {directorySeparator} </span>
                <span
                  className={
                    i !== arr.length - 1 || openedFile
                      ? 'cursor-pointer text-blue-200 hover:underline'
                      : 'text-gray-300'
                  }
                  onClick={() => {
                    setPath(subPath, false);
                  }}
                >
                  {p}
                </span>
              </span>
            );
          })}
      </p>

      {/* file name */}
      <p className="grow truncate text-gray-300">
        <span className="min-w-fit text-gray-300"> {directorySeparator}</span>
        {openedFile?.name}
      </p>
    </div>
  );
}
