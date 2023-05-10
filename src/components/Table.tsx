import { r } from '@tauri-apps/api/clipboard-2fa91cee';
import ButtonMenu from './ButtonMenu';
import { ButtonMenuConfig } from './ButtonMenu';

import clsx from 'clsx';

export interface TableColumn {
  field: string;
  headerName: string;
  element?: (rows: TableRow[], rowIndex: number) => React.ReactElement;
  className?: string;
}

export interface TableRow {
  [key: string]: React.ReactNode;
}

interface TableProps {
  rows: TableRow[];
  columns: TableColumn[];
  alignment?: 'even' | 'left';
  menuOptions?: ButtonMenuConfig;
  className?: string;
}

export function Table({
  rows,
  columns,
  alignment = 'even',
  menuOptions,
  className,
}: TableProps) {
  // If menuOptions is truthy, add an extra column for the menu buttons
  // Add a unique index for each ButtonMenu created to control respective rows
  // If alignment === 'left', add 'w-full' to className of last column
  const modifiedColumns = menuOptions
    ? [
        ...columns,
        {
          field: 'menu',
          headerName: '',
          element: (rows: TableRow[], rowIndex: number) => (
            <ButtonMenu
              tableRows={rows}
              rowIndex={rowIndex}
              menuItems={menuOptions.menuItems}
            />
          ),
          className: `text-end ${alignment === 'left' ? 'w-full' : ''}`,
        },
      ]
    : alignment == 'left'
    ? columns.map((column, index, array) =>
        index === array.length - 1 ? { ...column, className: 'w-full' } : column
      )
    : columns;

  // Code currently uses arbitrary values for min and max width when alignment is set to left
  return (
    <div>
      <table
        className={clsx(
          'z-10 w-full bg-gray-875 text-left tracking-medium',
          alignment === 'even' ? 'table-fixed' : 'table-auto',
          className
        )}
      >
        <thead className="h-6 border-b border-b-fade-700 bg-gray-875">
          <tr>
            {modifiedColumns.map((column, cIndex) => (
              <th
                key={cIndex}
                className={clsx(
                  'sticky top-0 z-30 bg-gray-875 px-4 py-2 text-medium font-bold',
                  alignment === 'left' && 'min-w-[8rem] max-w-[12rem]'
                )}
              >
                {column.headerName}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, indexRow) => (
            <tr
              key={indexRow}
              className={clsx(
                'h-full border-b border-b-fade-700',
                indexRow % 2 === 1 && 'bg-gray-850'
              )}
            >
              {modifiedColumns.map((column, indexColumn) => (
                <td
                  key={indexColumn}
                  className={clsx(
                    'p-4 text-left text-white/50',
                    column.className
                  )}
                >
                  {column.element
                    ? column.element(rows, indexRow)
                    : row[column.field]}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && (
        <div>
          <div className="mt-8 flex flex-col items-center justify-center">
            <div className=" text-h2 font-bold text-gray-400">
              No data currently available
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
