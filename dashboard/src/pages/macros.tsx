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
  useCallback,
  Dispatch,
  SetStateAction,
} from 'react';
import clsx from 'clsx';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'react-toastify';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import SettingField from 'components/SettingField';
import { SettingsCard } from 'components/Settings/SettingsBlocks';
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
import { noScriptingMessage, useCoreCapabilities } from 'data/SystemInfo';
import { MessageKey, t as translate, useI18n, useT } from 'i18n';

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
  const t = useT();
  return (
    <div className={clsx('mb-2', 'flex flex-col gap-4 @4xl:flex-row')}>
      <SettingsCard className="bg-gray-800">
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
      </SettingsCard>
      {macroData && macroData.error && (
        <TextCaption
          text={t('console.macros.configRequired')}
          className="text-small whitespace-pre-wrap text-red-300"
        />
      )}
    </div>
  );
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

export type MacrosPage = 'macros' | 'tasks' | 'history';
const pages: MacrosPage[] = ['macros', 'tasks', 'history'];
const pageLabels = {
  macros: 'console.macros.tab.macros',
  tasks: 'console.macros.tab.tasks',
  history: 'console.macros.tab.history',
} as const satisfies Record<MacrosPage, MessageKey>;

const Macros = () => {
  const { t, formatDateTime } = useI18n();
  useDocumentTitle(
    t('common.pageTitle', { page: t('console.macros.pageTitle') })
  );
  // a time from the core, in seconds since the epoch
  const formatUnixTime = useCallback(
    (unix: bigint | number | undefined | null) =>
      unix === undefined || unix === null
        ? t('console.macros.noTime')
        : formatDateTime(Number(unix) * 1000),
    [t, formatDateTime]
  );
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
      toast.error(
        translate('console.macros.loadFailed', {
          error: errorToString(fetchError),
        })
      );
  }, [fetchError]);

  const macros = useMemo(
    () =>
      (macrosQuery.data ?? []).map(
        (macro, i) =>
          ({
            id: i + 1,
            name: macro.name,
            last_run: formatUnixTime(macro.last_run),
            path: macro.path,
          }) as TableRow
      ),
    [macrosQuery.data, formatUnixTime]
  );

  const tasks = useMemo(
    () =>
      (tasksQuery.data ?? []).map(
        (task, i) =>
          ({
            id: i + 1,
            name: task.name,
            creation_time: formatUnixTime(task.creation_time),
            pid: task.pid,
          }) as TableRow
      ),
    [tasksQuery.data, formatUnixTime]
  );

  const history = useMemo(
    () =>
      (historyQuery.data ?? []).map(
        (entry, i) =>
          ({
            id: i + 1,
            name: entry.task.name,
            creation_time: formatUnixTime(entry.task.creation_time),
            finished: formatUnixTime(entry.exit_status.time),
            process_id: entry.task.pid,
          }) as TableRow
      ),
    [historyQuery.data, formatUnixTime]
  );

  const openMacroModal = async (row: TableRow) => {
    if (!canRun) return;
    if (!uuid) {
      toast.error(
        t('console.macros.openConfigFailed', {
          error: t('console.macros.noServer'),
        })
      );
      return;
    }
    try {
      const initialData = await getMacroConfig(uuid, row.name as string);
      // store initial settings to potentially rollback
      setInitialMacroData(initialData);
      setMacroData(initialData);
      setMacroRow(row);
    } catch (error) {
      toast.error(
        t('console.macros.openConfigFailed', { error: errorToString(error) })
      );
    }
  };

  const closeMacroModal = () => {
    setMacroRow(undefined);
    setMacroData(undefined);
    setInitialMacroData(undefined);
  };

  const [selectedPage, setSelectedPage] = useState<MacrosPage>('macros');

  // only the first load of a list; refetches keep showing the rows
  const pageLoading: Record<MacrosPage, boolean> = {
    macros: macrosQuery.isPending,
    tasks: tasksQuery.isPending,
    history: historyQuery.isPending,
  };

  const MacrosPageMap: Record<
    MacrosPage,
    { rows: TableRow[]; columns: TableColumn[]; menuOptions?: ButtonMenuConfig }
  > = useMemo(() => {
    return {
      macros: {
        rows: macros,
        columns: [
          { field: 'name', headerName: t('console.macros.column.name') },
          {
            field: 'last_run',
            headerName: t('console.macros.column.lastRun'),
          },
        ],
        menuOptions: {
          tableRows: macros,
          menuItems: [
            {
              label: t('console.macros.run'),
              icon: faPlayCircle,
              variant: 'text',
              intention: 'info',
              disabled: !canRun,
              onClick: async (row: TableRow) => {
                if (!uuid) {
                  toast.error(
                    t('console.macros.runFailed', {
                      error: t('console.macros.noServer'),
                    })
                  );
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
              label: t('console.macros.editConfig'),
              icon: faGear,
              variant: 'text',
              intention: 'info',
              disabled: !canRun,
              onClick: openMacroModal,
            },
          ],
        },
      },
      tasks: {
        rows: tasks,
        columns: [
          {
            field: 'name',
            headerName: t('console.macros.column.name'),
          },
          {
            field: 'creation_time',
            headerName: t('console.macros.column.created'),
          },
          {
            field: 'pid',
            headerName: t('console.macros.column.pid'),
          },
        ],
        menuOptions: {
          tableRows: tasks,
          menuItems: [
            {
              label: t('console.macros.killTask'),
              icon: faSkull,
              variant: 'text',
              intention: 'danger',
              disabled: false,
              onClick: async (row: TableRow) => {
                if (!uuid) {
                  toast.error(
                    t('console.macros.killFailed', {
                      error: t('console.macros.noServer'),
                    })
                  );
                  return;
                }
                // killTask returns an error string instead of throwing
                const error = await killTask(
                  queryClient,
                  uuid,
                  row.pid as string
                );
                if (error) {
                  toast.error(t('console.macros.killFailed', { error }));
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
      history: {
        rows: history,
        columns: [
          {
            field: 'name',
            headerName: t('console.macros.column.name'),
          },
          {
            field: 'creation_time',
            headerName: t('console.macros.column.created'),
          },
          {
            field: 'finished',
            headerName: t('console.macros.column.finished'),
          },
          {
            field: 'process_id',
            headerName: t('console.macros.column.pid'),
          },
        ],
      },
    };
  }, [macros, tasks, history, uuid, queryClient, canRun, t]);

  if (!macrosAvailable) {
    return (
      <div className="relative">
        <div className="mb-4 text-white/50">{noScriptingMessage()}</div>
      </div>
    );
  }

  if (!canManage) {
    return (
      <div className="relative">
        <div className="mb-4 text-white/50">
          {t('console.macros.ownerOnly')}
        </div>
      </div>
    );
  }

  return (
    <div className="relative">
      {!canRun && (
        <div className="mb-4">
          <WarningAlert>
            {t('console.macros.codeExecutionDisabledBanner')}
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
          title={t('console.macros.configTitle')}
          type="info"
          isOpen={true}
          onClose={closeMacroModal}
          confirmButtonText={t('common.save')}
          closeButtonText={t('common.close')}
          onConfirm={async () => {
            if (!canRun) {
              toast.error(t('console.macros.codeExecutionDisabled'));
              return;
            }
            if (!uuid || !macroData) {
              // theoretically, this should never occur
              toast.error(t('console.macros.invalidState'));
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
                t('console.macros.saveConfigFailed', {
                  error: errorToString(error),
                })
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
      <div className="mt-[0rem] mb-4">{t('console.macros.intro')}</div>
      <div className="flex flex-row flex-wrap justify-start border-b border-gray-400">
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
            {t(pageLabels[page])}
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
