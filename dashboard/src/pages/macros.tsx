import { useDocumentTitle } from 'usehooks-ts';
import { Table, TableColumn, TableRow } from 'components/Table';
import {
  faPlayCircle,
  faSkull,
  faGear,
} from '@fortawesome/free-solid-svg-icons';
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
import {
  useContext,
  useEffect,
  useState,
  useMemo,
  Dispatch,
  SetStateAction,
} from 'react';
import { MacroEntry } from 'bindings/MacroEntry';
import clsx from 'clsx';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'react-toastify';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { ReactNode } from 'react';
import SettingField from 'components/SettingField';
import { adaptSettingManifest } from 'components/Instance/InstanceSettingsCreate/SettingObject';
import { GetConfigResponse } from 'bindings/GetConfigResponse';
import { InstanceInfo } from 'bindings/InstanceInfo';
import TextCaption from 'components/Atoms/TextCaption';

const MacroModalContents = ({
  initialMacroData,
  selectedInstance,
  row,
  macroDataSetter,
}: {
  initialMacroData: GetConfigResponse;
  selectedInstance: InstanceInfo;
  row: TableRow;
  macroDataSetter: Dispatch<SetStateAction<GetConfigResponse | undefined>>;
}) => {
  // mirror state so settings ui is in sync with what gets sent to backend
  const [macroData, setMacroData] =
    useState<GetConfigResponse>(initialMacroData);
  return (
    <div className={clsx('mb-2', 'flex flex-col gap-4 @4xl:flex-row')}>
      <span className="w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {Object.entries(macroData.config).map(([_, manifest], index) => {
          return (
            <SettingField
              key={index}
              instance={selectedInstance}
              sectionId={`${selectedInstance.uuid}-${row.name}`}
              settingId={manifest.setting_id}
              setting={adaptSettingManifest(manifest)}
              error={null}
              onSubmit={(value) => {
                const newState = {
                  ...macroData,
                  config: {
                    ...macroData.config,
                    [`${manifest.name}`]: {
                      ...macroData.config[`${manifest.name}`],
                      value,
                    },
                  },
                };
                macroDataSetter(newState);
                setMacroData(newState);
              }}
            />
          );
        })}
      </span>
      {macroData && macroData.error && (
        <TextCaption
          text={'This macro requires additional config.' || ''}
          className="whitespace-pre-wrap text-small text-red-300"
        />
      )}
    </div>
  );
};

export type MacrosPage = 'All Macros' | 'Running Tasks' | 'History';
const Macros = () => {
  useDocumentTitle('Instance Macros - Lodestone');
  const { selectedInstance } = useContext(InstanceContext);
  const [macros, setMacros] = useState<TableRow[]>([]);
  const [tasks, setTasks] = useState<TableRow[]>([]);
  const [history, setHistory] = useState<TableRow[]>([]);
  const [showMacroConfigModal, setShowMacroConfigModal] = useState(false);
  const [macroConfigContents, setMacroConfigContents] =
    useState<ReactNode>(null);

  // macro settings state
  const [macroData, setMacroData] = useState<GetConfigResponse>();
  const [macroRow, setMacroRow] = useState<TableRow>();

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
      toast.error('Error opening macro config: No instance selected');
      return;
    }
    try {
      const initialData = await getMacroConfig(
        selectedInstance.uuid,
        row.name as string
      );
      // store initial settings to potentially rollback
      setMacroData(initialData);
      setMacroRow(row);

      setMacroConfigContents(
        <MacroModalContents
          initialMacroData={initialData}
          macroDataSetter={setMacroData}
          selectedInstance={selectedInstance}
          row={row}
        />
      );
      setShowMacroConfigModal(true);
    } catch (error) {
      // error potentialy thrown by axioswrapper - should be error type
      toast.error((error as Error).message);
    }
  };

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
                  toast.error(result);
                  await openMacroModal(row);
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
              onClick: openMacroModal,
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
      {showMacroConfigModal && (
        <ConfirmDialog
          title="Macro Config"
          type="info"
          isOpen={showMacroConfigModal}
          onClose={() => {
            setShowMacroConfigModal(false);
          }}
          confirmButtonText="Save"
          closeButtonText="Close"
          onConfirm={async () => {
            if (!selectedInstance || !macroRow || !macroData) {
              // theoretically, this should never occur
              toast.error('Error: Invalid state');
            } else {
              await storeMacroConfig(
                selectedInstance.uuid,
                macroRow.name as string,
                macroData.config
              );
            }

            setShowMacroConfigModal(false);
          }}
        >
          {macroConfigContents}
        </ConfirmDialog>
      )}
      <div className="mt-[0rem] mb-4">All macros for your instance</div>
      <div className="flex flex-row justify-start border-b border-gray-400">
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
