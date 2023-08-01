import { useDocumentTitle } from 'usehooks-ts';
import { Table, TableColumn, TableRow } from 'components/Table';
import { faPlayCircle, faSkull } from '@fortawesome/free-solid-svg-icons';
import { ButtonMenuConfig } from 'components/ButtonMenu';
import {
  createTask,
  getInstanceHistory,
  getMacros,
  getTasks,
  killTask,
} from 'utils/apis';
import { InstanceContext } from 'data/InstanceContext';
import { useContext, useMemo, useState } from 'react';
import clsx from 'clsx';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'react-toastify';

import { MacroEntry } from 'bindings/MacroEntry';
import { TaskEntry } from 'bindings/TaskEntry';
import { HistoryEntry } from 'bindings/HistoryEntry';

export type MacrosPage = 'All Macros' | 'Running Tasks' | 'History';
const Macros = () => {
  useDocumentTitle('Instance Macros - Lodestone');
  const { selectedInstance } = useContext(InstanceContext);
  const unixToFormattedTime = (unix: string | undefined) => {
    if (!unix) return 'N/A';
    const date = new Date(parseInt(unix) * 1000);
    return `${date
      .toLocaleDateString(undefined, {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
      })
      .replace(/,/, '')} at ${date.toLocaleTimeString(undefined, {
      hour: 'numeric',
      minute: '2-digit',
      hour12: true,
    })}`;
  };

  const queryClient = useQueryClient();

  const { data: macroEntry } = useQuery(
    ['instance', selectedInstance?.uuid, 'macroList'],
    () => getMacros(selectedInstance?.uuid as string),
    { enabled: !!selectedInstance, initialData: [], refetchOnMount: 'always' }
  );
  const macros = useMemo(() => {
    return macroEntry.map(
      (macro, i) =>
        ({
          id: i + 1,
          name: macro.name,
          last_run: unixToFormattedTime(macro.last_run?.toString()),
          path: macro.path,
        } as TableRow)
    );
  }, [macroEntry]);

  const { data: taskEntry } = useQuery(
    ['instance', selectedInstance?.uuid, 'taskList'],
    () => getTasks(selectedInstance?.uuid as string),
    { enabled: !!selectedInstance, initialData: [], refetchOnMount: 'always' }
  );
  const tasks = useMemo(() => {
    return taskEntry.map(
      (task, i) =>
        ({
          id: i + 1,
          name: task.name,
          creation_time: unixToFormattedTime(task.creation_time?.toString()),
          pid: task.pid,
        } as TableRow)
    );
  }, [taskEntry]);

  const { data: historyEntry } = useQuery(
    ['instance', selectedInstance?.uuid, 'historyList'],
    () => getInstanceHistory(selectedInstance?.uuid as string),
    {
      enabled: !!selectedInstance,
      initialData: [],
      refetchOnMount: 'always',
    }
  );
  const history = useMemo(() => {
    return historyEntry.map(
      (entry, i) =>
        ({
          id: i + 1,
          name: entry.task.name,
          creation_time: unixToFormattedTime(
            entry.task.creation_time.toString()
          ),
          finished: unixToFormattedTime(entry.exit_status.time.toString()),
          process_id: entry.task.pid,
        } as TableRow)
    );
  }, [historyEntry]);

  const [selectedPage, setSelectedPage] = useState<MacrosPage>('All Macros');

  const MacrosPageMap: Record<
    MacrosPage,
    { rows: TableRow[]; columns: TableColumn[]; menuOptions?: ButtonMenuConfig }
  > = useMemo(() => {
    return {
      'All Macros': {
        rows: macros,
        columns: [
          { field: 'name', headerName: 'MACRO NAME' },
          { field: 'last_run', headerName: 'LAST RUN' },
        ],
        menuOptions: {
          tableRows: macros,
          menuItems: [
            {
              label: 'Run Macro',
              icon: faPlayCircle,
              variant: 'text',
              intention: 'info',
              disabled: false,
              onClick: async (row: TableRow) => {
                if (!selectedInstance) {
                  toast.error('Error running macro: No instance selected');
                  return;
                }
                await createTask(
                  queryClient,
                  selectedInstance.uuid,
                  row.name as string,
                  []
                );

                queryClient.setQueryData(
                  ['instance', selectedInstance?.uuid, 'macroList'],
                  (oldData: MacroEntry[] | undefined) => {
                    return oldData === undefined
                      ? undefined
                      : oldData.map((macro) => {
                          if (macro.name !== row.name) {
                            return macro;
                          }
                          const newMacro = { ...macro };
                          newMacro.last_run = BigInt(
                            Math.floor(Date.now() / 1000).toString()
                          );
                          return newMacro;
                        });
                  }
                );
              },
            },
          ],
        },
      },
      'Running Tasks': {
        rows: tasks,
        columns: [
          {
            field: 'name',
            headerName: 'MACRO NAME',
          },
          {
            field: 'creation_time',
            headerName: 'CREATED',
          },
          {
            field: 'pid',
            headerName: 'PROCESS ID',
          },
        ],
        menuOptions: {
          tableRows: tasks,
          menuItems: [
            {
              label: 'Kill Task',
              icon: faSkull,
              variant: 'text',
              intention: 'danger',
              disabled: false,
              onClick: async (row: TableRow) => {
                if (!selectedInstance) {
                  toast.error('Error killing task: No instance selected');
                  return;
                }
                await killTask(
                  queryClient,
                  selectedInstance.uuid,
                  row.pid as string
                );

                let oldTask: TaskEntry | undefined;

                queryClient.setQueryData(
                  ['instance', selectedInstance?.uuid, 'taskList'],
                  (
                    oldData: TaskEntry[] | undefined
                  ): TaskEntry[] | undefined => {
                    return oldData === undefined
                      ? undefined
                      : oldData.filter((task) => {
                          const shouldKeep = task.pid !== row.pid;
                          if (!shouldKeep) {
                            oldTask = task;
                          }
                          return shouldKeep;
                        });
                  }
                );

                queryClient.setQueryData(
                  ['instance', selectedInstance?.uuid, 'historyList'],
                  (
                    oldData: HistoryEntry[] | undefined
                  ): HistoryEntry[] | undefined => {
                    if (oldTask === undefined) return oldData;

                    const newHistory: HistoryEntry = {
                      task: oldTask,
                      exit_status: {
                        type: 'Killed',
                        time: BigInt(Math.floor(Date.now() / 1000).toString()),
                      },
                    };
                    return oldData === undefined
                      ? [newHistory]
                      : [newHistory, ...oldData];
                  }
                );
              },
            },
          ],
        },
      },
      History: {
        rows: history,
        columns: [
          {
            field: 'name',
            headerName: 'MACRO NAME',
          },
          {
            field: 'creation_time',
            headerName: 'CREATED',
          },
          {
            field: 'finished',
            headerName: 'FINISHED',
          },
          {
            field: 'process_id',
            headerName: 'PROCESS ID',
          },
        ],
      },
    };
  }, [macros, tasks, history, selectedInstance, queryClient]);

  const pages: MacrosPage[] = ['All Macros', 'Running Tasks', 'History'];
  const Navbar = ({ pages }: { pages: MacrosPage[] }) => {
    return (
      <>
        <div className="flex flex-row justify-start">
          {pages.map((page) => (
            <button
              key={page}
              className={clsx(
                selectedPage === page &&
                  'border-b-2 border-blue-200 text-blue-200',
                'mr-4 font-mediumbold hover:cursor-pointer',
                'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50'
              )}
              onClick={() => setSelectedPage(page)}
            >
              {page}
            </button>
          ))}
        </div>
        <div className="w-full">
          <div className="h-[1px] bg-gray-400"></div>
        </div>
      </>
    );
  };

  return (
    <div className="relative">
      {/* <div className="absolute right-0 top-[-5rem]">
        <Button
          label="Create Macro"
          variant="text"
          intention="primary"
          className="float-right"
          onClick={() => setShowCreateMacro(true)}
        />
      </div> */}
      <div className="mt-[-3rem] mb-4">All macros for your instance</div>
      <Navbar pages={pages} />
      <div className="relative mx-auto mt-9 flex h-full w-full flex-row justify-center">
        <Table
          rows={MacrosPageMap[selectedPage].rows}
          columns={MacrosPageMap[selectedPage].columns}
          menuOptions={MacrosPageMap[selectedPage].menuOptions}
          alignment="even"
        />
      </div>
    </div>
  );
};

export default Macros;
