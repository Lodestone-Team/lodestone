import { useDocumentTitle, useEffectOnce } from 'usehooks-ts';
import { Table, TableColumn, TableRow } from 'components/Table';
import { faEdit, faSkull, faTrashCan } from '@fortawesome/free-solid-svg-icons';
import { ButtonMenuConfig } from 'components/ButtonMenu';
import {
  getMacros,
  getTasks,
  getInstanceHistory,
  createTask,
} from 'utils/apis';
import { InstanceContext } from 'data/InstanceContext';
import { useContext, useEffect, useState } from 'react';
import axios from 'axios';
import { MacroEntry } from 'bindings/MacroEntry';
import { TaskEntry } from 'bindings/TaskEntry';
import { HistoryEntry } from 'bindings/HistoryEntry';
import Button from 'components/Atoms/Button';
import clsx from 'clsx';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'react-toastify';

export type MacrosPage = 'All Macros' | 'Running Tasks' | 'History';
const Macros = () => {
  useDocumentTitle('Macros');
  const { selectedInstance } = useContext(InstanceContext);
  const [macros, setMacros] = useState<MacroEntry[]>([]);
  const [tasks, setTasks] = useState<TaskEntry[]>([]);
  const [history, setHistory] = useState<HistoryEntry[]>([]);

  console.log(selectedInstance);

  //   const createNewMacro = useCallback(
  //     (name, description, uuid) => {
  //       const newMacro = {
  //         name,
  //         description,
  //         uuid,
  //       };
  //       setMacros((prevMacros) => [...prevMacros, newMacro]);
  //     },
  //     [setMacros]
  //   );

  //   const createNewMacro = async () => {
  //     if (!selectedInstance) {
  //       toast.error('Error creating new macro: No instance selected');
  //       return;
  //     }
  //     await createTask(queryClient, selectedInstance.uuid, 'New Macro', []);
  //   };

  useEffect(() => {
    if (!selectedInstance) return;
    const fetchMacros = async (instanceUuid: string) => {
      const response: MacroEntry[] = await getMacros(instanceUuid);
      setMacros(response);
    };

    const fetchTasks = async (instanceUuid: string) => {
      const response = await getTasks(instanceUuid);
      setTasks(response);
    };

    const fetchHistory = async (instanceUuid: string) => {
      const response = await getInstanceHistory(instanceUuid);
      setHistory(response);
    };

    fetchMacros(selectedInstance.uuid);
    fetchTasks(selectedInstance.uuid);
    fetchHistory(selectedInstance.uuid);
  }, [selectedInstance]);

  const queryClient = useQueryClient();

  const columnsAnalog: TableColumn[] = [
    { field: 'make', headerName: 'MAKE' },
    { field: 'model', headerName: 'MODEL' },
    { field: 'lens', headerName: 'LENS' },
    { field: 'format', headerName: 'FORMAT' },
    { field: 'year', headerName: 'YEAR' },
  ];

  const rowsAnalog: TableRow[] = [
    {
      id: 1,
      make: 'Nikon',
      model: 'FM2',
      lens: '50mm f/1.8',
      format: '35mm',
      year: 1982,
    },
    {
      id: 2,
      make: 'Canon',
      model: 'AE-1',
      lens: '50mm f/1.4',
      format: '35mm',
      year: 1976,
    },
    {
      id: 3,
      make: 'Pentax',
      model: 'K1000',
      lens: '50mm f/2.0',
      format: '35mm',
      year: 1976,
    },
    {
      id: 4,
      make: 'Mamiya',
      model: 'RB67',
      lens: '127mm f/3.8',
      format: '120',
      year: 1970,
    },
    {
      id: 5,
      make: 'Hasselblad',
      model: '500CM',
      lens: '80mm f/2.8',
      format: '120',
      year: 1957,
    },
    {
      id: 6,
      make: 'Leica',
      model: 'M6',
      lens: '35mm f/2.0',
      format: '35mm',
      year: 1984,
    },
    {
      id: 7,
      make: 'Fuji',
      model: 'GW690III',
      lens: '90mm f/3.5',
      format: '120',
      year: 1980,
    },
    {
      id: 8,
      make: 'Minolta',
      model: 'X-700',
      lens: '50mm f/1.7',
      format: '35mm',
      year: 1981,
    },
    {
      id: 9,
      make: 'Rollei',
      model: '35T',
      lens: '40mm f/3.5',
      format: '35mm',
      year: 1960,
    },
    {
      id: 10,
      make: 'Kodak',
      model: 'Retina IIc',
      lens: '50mm f/2.8',
      format: '35mm',
      year: 1954,
    },
    {
      id: 11,
      make: 'Yashica',
      model: 'Mat-124G',
      lens: '80mm f/3.5',
      format: '120',
      year: 1970,
    },
    {
      id: 12,
      make: 'Voigtlander',
      model: 'Bessa R3A',
      lens: '40mm f/1.4',
      format: '35mm',
      year: 2004,
    },
    {
      id: 13,
      make: 'Zenza Bronica',
      model: 'SQ-Ai',
      lens: '80mm f/2.8',
      format: '120',
      year: 1982,
    },
    {
      id: 14,
      make: 'Konica',
      model: 'Hexar AF',
      lens: '35mm f/2.0',
      format: '35mm',
      year: 1993,
    },
    {
      id: 15,
      make: 'Zeiss Ikon',
      model: 'Contessa S310',
      lens: '45mm f/2.8',
      format: '35mm',
      year: 1957,
    },
    {
      id: 16,
      make: 'Fujifilm',
      model: 'Instax Mini 9',
      lens: '60mm f/12.7',
      format: 'Instant',
      year: 2017,
    },
    {
      id: 17,
      make: 'Polaroid',
      model: 'SX-70',
      lens: '116mm f/8',
      format: 'Polaroid',
      year: 1972,
    },
    {
      id: 18,
      make: 'Fujifilm',
      model: 'Instax Mini 90',
      lens: '60mm f/12.7',
      format: 'Instant',
      year: 2013,
    },
    {
      id: 19,
      make: 'Yashica',
      model: 'Mat-124G',
      lens: '80mm f/3.5',
      format: '120',
      year: 1970,
    },
    {
      id: 20,
      make: 'Holga',
      model: '120N',
      lens: '60mm f/8',
      format: '120',
      year: 1982,
    },
    {
      id: 21,
      make: 'Kodak',
      model: 'Brownie Hawkeye',
      lens: '75mm f/14.5',
      format: '620',
      year: 1949,
    },
    {
      id: 22,
      make: 'Rollei',
      model: '35',
      lens: '40mm f/3.5',
      format: '35mm',
      year: 1966,
    },
    {
      id: 23,
      make: 'Agfa',
      model: 'Clack',
      lens: '95mm f/11',
      format: '120',
      year: 1954,
    },
    {
      id: 24,
      make: 'Lomography',
      model: 'Diana F+',
      lens: '75mm f/8',
      format: '120',
      year: 2007,
    },
    {
      id: 25,
      make: 'Pentax',
      model: '645',
      lens: '75mm f/2.8',
      format: '120',
      year: 1984,
    },
    {
      id: 26,
      make: 'Nimslo',
      model: 'Nimslo 3D',
      lens: '30mm f/5.6',
      format: '35mm',
      year: 1980,
    },
    {
      id: 27,
      make: 'Voigtlander',
      model: 'Bessa R2A',
      lens: '50mm f/1.5',
      format: '35mm',
      year: 2004,
    },
    {
      id: 28,
      make: 'Fujifilm',
      model: 'GFX 50S',
      lens: '63mm f/2.8',
      format: 'Medium Format',
      year: 2017,
    },
    {
      id: 29,
      make: 'Bronica',
      model: 'SQ-A',
      lens: '80mm f/2.8',
      format: 'Medium Format',
      year: 1980,
    },
    {
      id: 30,
      make: 'Minox',
      model: '35 EL',
      lens: '35mm f/2.8',
      format: '35mm',
      year: 1974,
    },
  ];

  const menuItems1: ButtonMenuConfig = {
    tableRows: rowsAnalog,
    menuItems: [
      {
        label: 'Edit in file viewer',
        icon: faEdit,
        variant: 'text',
        intention: 'info',
        disabled: false,
        onClick: (row: TableRow) =>
          console.log(`Edit on ${row.id}: ${row.make} ${row.model}`),
      },
      {
        label: 'another one',
        icon: faSkull,
        variant: 'text',
        intention: 'info',
        disabled: false,
        onClick: (row: TableRow) =>
          console.log(`Another on ${row.id}: ${row.make} ${row.model}`),
      },
      {
        label: 'Obliterate',
        icon: faTrashCan,
        variant: 'text',
        intention: 'danger',
        disabled: false,
        onClick: (row: TableRow) =>
          console.log(`Obliterate on ${row.id}: ${row.make} ${row.model}`),
      },
    ],
  };

  const [selectedPage, setSelectedPage] = useState<MacrosPage>('All Macros');
  const MacrosPageMap = {
    'All Macros': {
      rows: rowsAnalog,
      columns: columnsAnalog,
      menuOptions: menuItems1,
    },
    'Running Tasks': {
      rows: rowsAnalog,
      columns: columnsAnalog,
      menuOptions: menuItems1,
    },
    History: {
      rows: [],
      columns: [],
      menuOptions: menuItems1,
    },
  };

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
                'mr-4 font-mediumbold hover:cursor-pointer'
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
    // used to possibly center the content
    <div className="relative">
      <div className="absolute right-0 top-[-5rem]">
        <Button
          label="Create Macro"
          variant="text"
          intention="primary"
          className="float-right"
          onClick={() => console.log('Create Macro')}
        />
      </div>
      <div className="mt-[-3rem] mb-4">All macros for your instance</div>
      <Navbar pages={pages} />
      <div className="relative mx-auto mt-9 flex h-full w-full flex-row justify-center">
        <Table
          rows={MacrosPageMap[selectedPage].rows}
          columns={MacrosPageMap[selectedPage].columns}
          menuOptions={MacrosPageMap[selectedPage].menuOptions}
          alignment="left"
        />
      </div>
    </div>
  );
};

export default Macros;
