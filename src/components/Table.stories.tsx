import React from 'react';

import { ComponentStory, ComponentMeta } from '@storybook/react';
import {Table, TableColumn, TableRow} from "./Table";

const mockTableColumns: TableColumn[] = [
    { field: "name", headerName: "TASK NAME", width: 200, className: "font-mono text-white" },
    { field: "start", headerName: "CREATED", width: 200 },
    { field: "finish", headerName: "FINISHED", width: 200 },
    { field: "pId", headerName: "PROCESS ID", width: 200 },
]

const mockTableRows: TableRow[] = [
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
    { name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890 },
]

export default {
    title: 'Table',
    component: Table,
} as ComponentMeta<typeof Table>;

const Template: ComponentStory<typeof Table> = (args) =>
    (
        <Table {...args}/>
    )

export const Primary = Template.bind({});


Primary.args = {
    columns: mockTableColumns,
    rows: mockTableRows,
    className: "w-96 h-96",
}