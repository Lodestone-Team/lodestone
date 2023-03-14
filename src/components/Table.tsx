import * as React from 'react';
import {cn} from "../utils/util";


export interface TableColumn {
    field: string;
    headerName: string;
    width: number;
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
        <div className={cn("overflow-overlay flex h-fit max-h-full w-full flex-col overflow-x-auto bg-gray-875", className)}>
            <div className="flex h-fit w-max min-w-full flex-col">
                <div className="flex h-6 min-h-[24px] items-center gap-2.5 self-stretch overflow-x-hidden border-b border-b-fade-700 pl-3">
                    {columns.map((column, cIndex) => {
                            return (
                                <div className="" key={cIndex} style={{width: column.width}}>
                                    <body className="text-small font-bold tracking-medium">{column.headerName}</body>
                                </div>
                            )
                        }
                    )}
                </div>

                <div className={`flex scroll-pr-0 flex-col overflow-x-hidden overflow-y-scroll`}>
                    {rows.map((row, index) => {

                        return (
                            <div
                                className={"flex flex-row self-stretch items-center px-3 py-2.5 gap-2.5 h-10 border-b border-b-fade-700 " + (index % 2 === 1 ? "bg-gray-850" : "")}
                                key={index}>
                                {columns.map((column, cIndex) => {
                                        return (
                                            <div key={cIndex} style={{width: column.width}}>
                                                <span
                                                    className={cn("whitespace-nowrap text-small tracking-medium text-white/50", column.className)}>{row[column.field]}</span>
                                            </div>
                                        )
                                    }
                                )}
                            </div>
                        )
                    })}
                </div>
            </div>
        </div>
    );
}