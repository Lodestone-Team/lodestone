import * as React from 'react';
import {cn} from "../utils/util";
import clsx from "clsx";


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
        <div className={cn("flex h-fit max-h-full w-full flex-col overflow-auto bg-gray-875", className)}>
            <div className="flex h-fit w-max min-w-full flex-col">
                <div
                    className="sticky top-0 flex h-6 items-center gap-2.5 self-stretch overflow-hidden border-b border-b-fade-700 bg-gray-875 px-3">
                    {columns.map((column, cIndex) => (
                            <div key={cIndex} style={{width: column.width}}>
                                <span className="text-small font-bold tracking-medium">{column.headerName}</span>
                            </div>
                        )
                    )}
                </div>

                {rows.map((row, index) => {

                    return (
                        <div
                            className={clsx("flex h-10 flex-row items-center gap-2.5 border-b border-b-fade-700 px-3 py-2.5", index % 2 === 1 && "bg-gray-850")}
                            key={index}>
                            {columns.map((column, cIndex) => (
                                    <div key={cIndex} style={{width: column.width}}>
                                        <span
                                            className={cn("whitespace-nowrap text-small tracking-medium text-white/50", column.className)}>{row[column.field]}</span>
                                    </div>
                                )
                            )}
                        </div>
                    )
                })}
            </div>
        </div>
    );
}