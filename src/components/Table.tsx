import * as React from 'react';
import clsx from "clsx";

import Menu from './ButtonMenu';
import { MenuItemProps } from './ButtonMenu';

export interface TableColumn {
  field: string;
  headerName: string;
  className?: string;
}

export interface TableRow {
  [key: string]: React.ReactNode;
}

interface TableProps {
  rows: TableRow[];
  columns: TableColumn[];
  menuOptions?: MenuItemProps;
  className?: string;
}

export function Table({rows, columns, menuOptions, className}: TableProps) {
  

  return (
    <table className={clsx("table-auto whitespace-nowrap bg-gray-875 text-left tracking-medium", className)}>
      <thead className="h-6 border-b border-b-fade-700 bg-gray-875">
      <tr>
        {columns.map((column, cIndex) => (
            <th key={cIndex}
              className="sticky top-0 bg-gray-875 px-3 text-small font-bold">
              {column.headerName}
            </th>
          )
        )}
      </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => {
          return (
            <tr
              className={clsx("h-12 border-b border-b-fade-700", index % 2 === 1 && "bg-gray-850")}
              key={index}
            >
              {columns.map((column, cIndex) => (
                <td key={cIndex}
                className={clsx("px-3 text-left text-white/50", column.className)}>
                  {row[column.field]}
                </td>
              ))}
              <td>
              {menuOptions ? (
                <Menu
                  menuItems={menuOptions.menuItems}
                />
              ) : null}
            </td>
            </tr>
          )
        })}
        
      </tbody>
    </table>
  );
}
