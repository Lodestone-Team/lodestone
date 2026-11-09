import ButtonMenu from './ButtonMenu';
import { ButtonMenuConfig } from './ButtonMenu';

import clsx from 'clsx';
import { Skeleton } from './Atoms/Skeleton';
import { useT } from 'i18n';

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
  /** the rows are still loading: no "No data" message meanwhile */
  isLoading?: boolean;
}

export function Table({
  rows,
  columns,
  alignment = 'even',
  menuOptions,
  className,
  isLoading = false,
}: TableProps) {
  const t = useT();
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
          index === array.length - 1
            ? { ...column, className: 'w-full' }
            : column
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
                  alignment === 'left' && 'max-w-[12rem] min-w-[8rem]'
                )}
              >
                {column.headerName}
              </th>
            ))}
          </tr>
        </thead>
        <tbody
          className={clsx(rows.length === 0 && isLoading && 'loading-delayed')}
          aria-busy={rows.length === 0 && isLoading}
        >
          {rows.length === 0 &&
            isLoading &&
            // rows of the table's own shape while it loads (after a moment)
            Array.from({ length: 5 }, (_, indexRow) => (
              <tr
                key={indexRow}
                className={clsx(
                  'border-b border-b-fade-700',
                  indexRow % 2 === 1 && 'bg-gray-850'
                )}
              >
                {modifiedColumns.map((_, indexColumn) => (
                  <td key={indexColumn} className="p-4">
                    <Skeleton
                      className={clsx(
                        'h-3.5 rounded-sm',
                        ['w-3/4', 'w-1/2', 'w-2/3'][
                          (indexRow + indexColumn) % 3
                        ]
                      )}
                    />
                  </td>
                ))}
              </tr>
            ))}
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
      {rows.length === 0 && !isLoading && (
        <div>
          <div className="mt-8 flex flex-col items-center justify-center">
            <div className="text-h2 font-bold text-gray-400">
              {t('common.table.noData')}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
