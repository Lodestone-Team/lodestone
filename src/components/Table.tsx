import * as React from 'react';
import {cn} from "../utils/util";
import clsx from "clsx";

import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faEllipsisVertical } from '@fortawesome/free-solid-svg-icons';

export interface TableColumn {
  field: string;
  headerName: string;
  className?: string;
}

export interface TableRow {
  [key: string]: React.ReactNode;
}

interface CardProps {
  rows: TableRow[];
  columns: TableColumn[];
  className?: string;
}

export function Table({rows, columns, className}: CardProps) {
  return (
    <table className={cn("table-auto whitespace-nowrap bg-gray-875 text-left tracking-medium", className)}>
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
            className={clsx("h-10 border-b border-b-fade-700", index % 2 === 1 && "bg-gray-850")}
            key={index}
          >
            {columns.map((column, cIndex) => (
              <td key={cIndex}
              className={cn("px-3 text-left text-white/50", column.className)}>
                {row[column.field]}
              </td>
            ))}
            <td>
              <button>
                <FontAwesomeIcon icon={faEllipsisVertical}/>
              </button>
            </td>
          </tr>
        )
      })}
      </tbody>
    </table>
  );
}