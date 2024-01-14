import { useDocumentTitle } from 'usehooks-ts';
import { Table, TableColumn, TableRow } from 'components/Table';
import { faPlayCircle, faSkull, faGear } from '@fortawesome/free-solid-svg-icons';
import { ButtonMenuConfig } from 'components/ButtonMenu';
import {
  getMacros,
  getTasks,
  getInstanceHistory,
  createTask,
  killTask,
  getMacroConfig,
  storeMacroConfig,
} from 'utils/apis';
import { InstanceContext } from 'data/InstanceContext';
import { useContext, useEffect, useState, useMemo } from 'react';
import { MacroEntry } from 'bindings/MacroEntry';
import clsx from 'clsx';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'react-toastify';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { ReactNode } from 'react';
import { FieldFromManifest } from 'components/Instance/Create/FieldFromManifest';
import { FormFromManifest } from 'components/Instance/Create/FormFromManifest';
import SettingField from 'components/SettingField';
import { SettingFieldObject, adaptSettingManifest } from 'components/Instance/InstanceSettingsCreate/SettingObject';

export type MacrosPage = 'All Macros' | 'Running Tasks' | 'History';
const Macros = () => {
  useDocumentTitle('Instance Macros - Lodestone');
  const { selectedInstance } = useContext(InstanceContext);
  const [macros, setMacros] = useState<TableRow[]>([]);
  const [tasks, setTasks] = useState<TableRow[]>([]);
  const [history, setHistory] = useState<TableRow[]>([]);
  const [ showMacroConfigModal, setShowMacroConfigModal ] = useState(false);
  const [macroConfigContents, setMacroConfigContents] = useState<ReactNode>(null);

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

  const fetchMacros = async (instanceUuid: string) => {
    const response: MacroEntry[] = await getMacros(instanceUuid);
    setMacros(
      response.map(
        (macro, i) =>
          ({
            id: i + 1,
            name: macro.name,
            last_run: unixToFormattedTime(macro.last_run?.toString()),
            path: macro.path,
          } as TableRow)
      )
    );
  };

  const fetchTasks = async (instanceUuid: string) => {
    const response = await getTasks(instanceUuid);
    setTasks(
      response.map(
        (task, i) =>
          ({
            id: i + 1,
            name: task.name,
            creation_time: unixToFormattedTime(task.creation_time.toString()),
            pid: task.pid,
          } as TableRow)
      )
    );
  };

  const fetchHistory = async (instanceUuid: string) => {
    const response = await getInstanceHistory(instanceUuid);
    setHistory(
      response.map(
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
      )
    );
  };

  const openMacroModal = async (row: TableRow) => {
    if (!selectedInstance) {
      toast.error('Error running macro: No instance selected');
      return;
    }
    const macroData = await getMacroConfig(selectedInstance.uuid,  row.name as string);
    setMacroConfigContents(
      <span>
        {
          Object.entries(macroData.config).map(([_, manifest], index) => {
            return (<>
              <SettingField 
                key = {index} 
                instance = {selectedInstance}
                sectionId={`${selectedInstance.uuid}-${row.name}`}
                settingId={manifest.setting_id}
                setting={adaptSettingManifest(manifest)}
                error = {null}
                onSubmit={async (value) => {
                  const newSettings = macroData.config;
                  newSettings[`${manifest.name}`].value = value
                  const result = await storeMacroConfig(
                    selectedInstance.uuid,
                    row.name as string,
                    newSettings
                  );
                  console.log({result})
                }}
              />
              <br></br>
            </>)
          })
        }
      </span>
    )
    setShowMacroConfigModal(true)
  }

  useEffect(() => {
    if (!selectedInstance) return;

    const fetchAll = async () => {
      if (!selectedInstance) return;
      fetchMacros(selectedInstance.uuid);
      fetchTasks(selectedInstance.uuid);
      fetchHistory(selectedInstance.uuid);
    };

    fetchAll();
  }, [selectedInstance]);

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
                const result = await createTask(
                  queryClient,
                  selectedInstance.uuid,
                  row.name as string,
                  []
                );
                if (result !== '') {
                  await openMacroModal(row)
                  return;
                }
                const newMacros = macros.map((macro) => {
                  if (macro.name !== row.name) {
                    return macro;
                  }
                  const newMacro = { ...macro };
                  newMacro.last_run = unixToFormattedTime(
                    Math.floor(Date.now() / 1000).toString()
                  );
                  return newMacro;
                });
                setMacros(newMacros);
                fetchTasks(selectedInstance.uuid);
              },
            },
            {
              label: 'Edit Config',
              icon: faGear,
              variant: 'text',
              intention: 'info',
              disabled: false,
              onClick: openMacroModal
            }
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
                setTasks(tasks.filter((task) => task.id !== row.id)); //rather than refetching, we just update the display
                const newHistory = {
                  id: row.id,
                  name: row.name,
                  creation_time: row.creation_time,
                  finished: unixToFormattedTime(
                    Math.floor(Date.now() / 1000).toString()
                  ), //unix time in seconds
                  process_id: row.pid,
                };
                setHistory([newHistory, ...history]);
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
      {
      showMacroConfigModal && 
      <ConfirmDialog
      title = "Macro Config"
      type = "info"
      isOpen = {showMacroConfigModal}
      onClose = {() => {
        setShowMacroConfigModal(false)
      }}
      confirmButtonText='Close'
      >
        {macroConfigContents}
      </ConfirmDialog>
      }
      <div className="mt-[-3rem] mb-4">All macros for your instance</div>
      <div className="flex flex-row justify-start border-b border-gray-400">
        {pages.map((page) => (
          <button
            key={page}
            className={clsx(
              selectedPage === page &&
                'border-b-2 border-blue-200 text-blue-200',
              'font-mediumbold mr-4 hover:cursor-pointer',
              'enabled:focus-visible:ring-blue-faded/50 focus-visible:outline-none enabled:focus-visible:ring-4'
            )}
            onClick={() => setSelectedPage(page)}
          >
            {page}
          </button>
        ))}
      </div>
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
