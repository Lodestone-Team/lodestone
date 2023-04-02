import * as React from 'react';
import clsx from "clsx";

import ButtonMenu from './ButtonMenu';
import { ButtonMenuProps } from './ButtonMenu';

export interface TableColumn {
  field: string;
  headerName: string;
  element?: (row: TableRow) => React.ReactElement;
  className?: string;
}

export interface TableRow {
  [key: string]: React.ReactNode;
}

interface TableProps {
  rows: TableRow[];
  columns: TableColumn[];
  alignment?: 'even' | 'left';
  menuOptions?: ButtonMenuProps;
  className?: string;
}

export function Table({rows, columns, alignment = 'even', menuOptions, className}: TableProps) {
  
  // If menuOptions is truthy, add an extra column for the menu buttons
  // If alignment === 'left', add 'w-full' to className
  const modifiedColumns = menuOptions ? [...columns, {
    field: 'menu',
    headerName: '',
    element: () => <ButtonMenu menuItems={menuOptions.menuItems} />,
    className: `text-end ${alignment === 'left' ? 'w-full' : ''}`,
  }] : columns;

  // Code currently uses arbitrary values for min and max width when alignment is set to left
  return (
    <table className={clsx("w-full bg-gray-875 text-left tracking-medium", alignment === 'even' ? "table-fixed" : "table-auto", className)}>
      <thead className="h-6 border-b border-b-fade-700 bg-gray-875">
        <tr>
          {modifiedColumns.map((column, cIndex) => (
            <th
              key={cIndex}
              className={clsx("sticky top-0 z-20 bg-gray-875 px-4 py-2 text-medium font-bold", alignment === 'left' && "min-w-[8rem] max-w-[12rem]")}
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
            className={clsx("h-full border-b border-b-fade-700", indexRow % 2 === 1 && "bg-gray-850")}
          >
            {modifiedColumns.map((column, indexColumn) => (
              <td
                key={indexColumn}
                className={clsx("p-4 text-left text-white/50", column.className)}
              >
                {column.element ? column.element(row) : row[column.field]}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
