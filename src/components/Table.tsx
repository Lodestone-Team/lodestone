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
  menuOptions?: ButtonMenuProps;
  className?: string;
}

export function Table({rows, columns, menuOptions, className}: TableProps) {
  // If menuOptions is truthy, add an extra column for the menu buttons
  const modifiedColumns = menuOptions ? [...columns, {
    field: 'menu',
    headerName: '',
    element: () => <ButtonMenu menuItems={menuOptions.menuItems} />,
    className: 'text-end',
  }] : columns;

  return (
    <table className={clsx("table-fixed whitespace-nowrap bg-gray-875 text-left tracking-medium", className)}>
      <thead className="h-6 border-b border-b-fade-700 bg-gray-875">
      <tr>
        {modifiedColumns.map((column, cIndex) => (
            <th
              key={cIndex}
              className="sticky top-0 bg-gray-875 px-4 text-medium font-bold"
            >
              {column.headerName}
            </th>
          )
        )}
      </tr>
      </thead>
      <tbody>
        {rows.map((row, indexRow) => (
          <tr
            key={indexRow}
            className={clsx("h-12 border-b border-b-fade-700", indexRow % 2 === 1 && "bg-gray-850")}
          >
            {modifiedColumns.map((column, indexColumn) => (
              <td
                key={indexColumn}
                className={clsx("px-4 text-left text-white/50", column.className)}
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
