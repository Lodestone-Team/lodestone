import React from 'react';

import {ComponentStory, ComponentMeta} from '@storybook/react';
import {Table, TableColumn, TableRow} from "./Table";
import {cn} from "../utils/util";

const mockTableColumns: TableColumn[] = [
    {field: "name", headerName: "MACRO NAME", className: "font-mono text-white"},
    {field: "start", headerName: "CREATED"},
    {field: "finish", headerName: "FINISHED"},
    {field: "pId", headerName: "PROCESS ID"},
]

const mockTableRows: TableRow[] = [
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
    {name: "Hello.js", start: "Jan 4 2023, 09:37", finish: "Jan 4 2023, 09:37", pId: 1234567890},
]

export default {
    title: 'library/Table',
    component: Table,
} as ComponentMeta<typeof Table>;

const Template: ComponentStory<typeof Table> = (args) =>
    (
        <Table {...args}/>
    )
const TemplateDiv: ComponentStory<typeof Table> = (args) =>
    (
        <div className={cn("flex h-fit max-h-full w-full flex-col overflow-auto bg-gray-875", args.className)}>
            <Table {...args}/>
        </div>
    )

export const Primary = Template.bind({});
export const Minimal = Template.bind({});
export const Clip = TemplateDiv.bind({});


Primary.args = {
    columns: mockTableColumns,
    rows: mockTableRows.slice(0, 5),
    className: "w-[600px]",
}
Minimal.args = {
    columns: mockTableColumns,
    rows: mockTableRows.slice(0, 5),
    className: "w-full",
}
Clip.args = {
    columns: mockTableColumns,
    rows: mockTableRows,
    className: "w-96 h-96",
}
