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
import clsx from 'clsx';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'react-toastify';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import SettingField from 'components/SettingField';
import { adaptSettingManifest } from 'components/Instance/InstanceSettingsCreate/SettingObject';
import { GetConfigResponse } from 'bindings/GetConfigResponse';
import { InstanceInfo } from 'bindings/InstanceInfo';
import TextCaption from 'components/Atoms/TextCaption';
import { errorToString } from 'utils/util';
import WarningAlert from 'components/Atoms/WarningAlert';
import {
  isCodeExecutionDisabledError,
  useCodeExecution,
} from 'data/CodeExecution';
import { NO_SCRIPTING_MESSAGE, useCoreCapabilities } from 'data/SystemInfo';

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
      <span className="w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
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
          text={'This macro requires additional config.'}
          className="whitespace-pre-wrap text-small text-red-300"
        />
      )}
    </div>
  );
};

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

// Query keys for the macro lists of an instance. The event stream should
// invalidate these on macro Started/Stopped events.
const macroListKey = (uuid: string | undefined) => [
  'instance',
  uuid,
  'macroList',
];
const taskListKey = (uuid: string | undefined) => [
  'instance',
  uuid,
  'taskList',
];
const historyListKey = (uuid: string | undefined) => [
  'instance',
  uuid,
  'historyList',
];

export type MacrosPage = 'All Macros' | 'Running Tasks' | 'History';
const Macros = () => {
  useDocumentTitle('Server Macros - Lodestone');
  const { selectedInstance } = useContext(InstanceContext);
  const uuid = selectedInstance?.uuid;
  const queryClient = useQueryClient();
  // macros are owner only; running or configuring them also needs code
  // execution to be enabled (listing, killing and history still work)
  const { canManage, canRun } = useCodeExecution();
  // a core built without Deno has no macros at all; the lists wait until we know
  const { macros: macrosAvailable, isPending: capabilitiesPending } =
    useCoreCapabilities();
  const listsEnabled =
    !!uuid && canManage && macrosAvailable && !capabilitiesPending;

  // macro settings state; the modal is open while macroRow is set
  const [macroData, setMacroData] = useState<GetConfigResponse>();
  const [initialMacroData, setInitialMacroData] = useState<GetConfigResponse>();
  const [macroRow, setMacroRow] = useState<TableRow>();

  // Keyed by uuid, so player and state events (which replace the instance
  // object) don't refetch, and a response for another instance can't land
  // in this one's list. staleTime 0 refetches whenever the page is opened.
  const macrosQuery = useQuery({
    queryKey: macroListKey(uuid),
    queryFn: () => getMacros(uuid as string),
    enabled: listsEnabled,
    staleTime: 0,
  });
  const tasksQuery = useQuery({
    queryKey: taskListKey(uuid),
    queryFn: () => getTasks(uuid as string),
    enabled: listsEnabled,
    staleTime: 0,
  });
  const historyQuery = useQuery({
    queryKey: historyListKey(uuid),
    queryFn: () => getInstanceHistory(uuid as string),
    enabled: listsEnabled,
    staleTime: 0,
  });

  const fetchError =
    macrosQuery.error ?? tasksQuery.error ?? historyQuery.error;
  useEffect(() => {
    if (fetchError)
      toast.error(`Failed to load macros: ${errorToString(fetchError)}`);
  }, [fetchError]);

  const macros = useMemo(
    () =>
      (macrosQuery.data ?? []).map(
        (macro, i) =>
          ({
            id: i + 1,
            name: macro.name,
            last_run: unixToFormattedTime(macro.last_run?.toString()),
            path: macro.path,
          }) as TableRow
      ),
    [macrosQuery.data]
  );

  const tasks = useMemo(
    () =>
      (tasksQuery.data ?? []).map(
        (task, i) =>
          ({
            id: i + 1,
            name: task.name,
            creation_time: unixToFormattedTime(task.creation_time.toString()),
            pid: task.pid,
          }) as TableRow
      ),
    [tasksQuery.data]
  );

  const history = useMemo(
    () =>
      (historyQuery.data ?? []).map(
        (entry, i) =>
          ({
            id: i + 1,
            name: entry.task.name,
            creation_time: unixToFormattedTime(
              entry.task.creation_time.toString()
            ),
            finished: unixToFormattedTime(entry.exit_status.time.toString()),
            process_id: entry.task.pid,
          }) as TableRow
      ),
    [historyQuery.data]
  );

  const openMacroModal = async (row: TableRow) => {
    if (!canRun) return;
    if (!uuid) {
      toast.error('Error opening macro config: No server selected');
      return;
    }
    try {
      const initialData = await getMacroConfig(uuid, row.name as string);
      // store initial settings to potentially rollback
      setInitialMacroData(initialData);
      setMacroData(initialData);
      setMacroRow(row);
    } catch (error) {
      toast.error(`Error opening macro config: ${errorToString(error)}`);
    }
  };

  const closeMacroModal = () => {
    setMacroRow(undefined);
    setMacroData(undefined);
    setInitialMacroData(undefined);
  };

  const [selectedPage, setSelectedPage] = useState<MacrosPage>('All Macros');

  // only the first load of a list; refetches keep showing the rows
  const pageLoading: Record<MacrosPage, boolean> = {
    'All Macros': macrosQuery.isPending,
    'Running Tasks': tasksQuery.isPending,
    History: historyQuery.isPending,
  };

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
              disabled: !canRun,
              onClick: async (row: TableRow) => {
                if (!uuid) {
                  toast.error('Error running macro: No server selected');
                  return;
                }
                const result = await createTask(
                  queryClient,
                  uuid,
                  row.name as string,
                  []
                );
                if (result !== '') {
                  toast.error(result);
                  if (isCodeExecutionDisabledError(result)) {
                    // the switch was turned off since we last checked
                    queryClient.invalidateQueries({
                      queryKey: ['global_settings'],
                    });
                    return;
                  }
                  await openMacroModal(row);
                  return;
                }
                // the run changed the macro's last run time and the task list
                queryClient.invalidateQueries({ queryKey: macroListKey(uuid) });
                queryClient.invalidateQueries({ queryKey: taskListKey(uuid) });
              },
            },
            {
              label: 'Edit Config',
              icon: faGear,
              variant: 'text',
              intention: 'info',
              disabled: !canRun,
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
                if (!uuid) {
                  toast.error('Error killing task: No server selected');
                  return;
                }
                // killTask returns an error string instead of throwing
                const error = await killTask(
                  queryClient,
                  uuid,
                  row.pid as string
                );
                if (error) {
                  toast.error(`Failed to kill task: ${error}`);
                  return;
                }
                queryClient.invalidateQueries({ queryKey: taskListKey(uuid) });
                queryClient.invalidateQueries({
                  queryKey: historyListKey(uuid),
                });
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
  }, [macros, tasks, history, uuid, queryClient, canRun]);

  const pages: MacrosPage[] = ['All Macros', 'Running Tasks', 'History'];

  if (!macrosAvailable) {
    return (
      <div className="relative">
        <div className="mb-4 text-white/50">{NO_SCRIPTING_MESSAGE}</div>
      </div>
    );
  }

  if (!canManage) {
    return (
      <div className="relative">
        <div className="mb-4 text-white/50">
          Macros are only available to the owner of this core.
        </div>
      </div>
    );
  }

  return (
    <div className="relative">
      {!canRun && (
        <div className="mb-4">
          <WarningAlert>
            Code execution is disabled on this core. Macros can&apos;t be run or
            configured until it is turned back on in Core Settings.
          </WarningAlert>
        </div>
      )}
      {/* <div className="absolute right-0 top-[-5rem]">
        <Button
          label="Create Macro"
          variant="text"
          intention="primary"
          className="float-right"
          onClick={() => setShowCreateMacro(true)}
        />
      </div> */}
      {macroRow && selectedInstance && initialMacroData && (
        <ConfirmDialog
          title="Macro Config"
          type="info"
          isOpen={true}
          onClose={closeMacroModal}
          confirmButtonText="Save"
          closeButtonText="Close"
          onConfirm={async () => {
            if (!canRun) {
              toast.error('Code execution is disabled on this core');
              return;
            }
            if (!uuid || !macroData) {
              // theoretically, this should never occur
              toast.error('Error: Invalid state');
              return;
            }
            try {
              await storeMacroConfig(
                uuid,
                macroRow.name as string,
                macroData.config
              );
            } catch (error) {
              // keep the dialog open so the changes aren't lost
              toast.error(
                `Failed to save macro config: ${errorToString(error)}`
              );
              return;
            }
            closeMacroModal();
          }}
        >
          <MacroModalContents
            initialMacroData={initialMacroData}
            macroDataSetter={setMacroData}
            selectedInstance={selectedInstance}
            row={macroRow}
          />
        </ConfirmDialog>
      )}
      <div className="mt-[0rem] mb-4">All macros for your server</div>
      <div className="flex flex-row justify-start border-b border-gray-400">
        {pages.map((page) => (
          <button
            key={page}
            className={clsx(
              selectedPage === page &&
                'border-b-2 border-blue-200 text-blue-200',
              'mr-4 font-mediumbold hover:cursor-pointer',
              'focus-visible:outline-hidden enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50'
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
          isLoading={pageLoading[selectedPage]}
        />
      </div>
    </div>
  );
};

export default Macros;
