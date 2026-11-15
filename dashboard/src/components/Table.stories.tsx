import type {Meta, StoryObj} from '@storybook/nextjs-vite';
import {Table, TableColumn, TableRow} from "./Table";
import {cn} from "../utils/util";

const mockTableColumns: TableColumn[] = [
    {field: "name", headerName: "NAME", className: "font-mono text-white"},
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

const meta: Meta<typeof Table> = {
    title: 'library/Table',
    component: Table,
};
export default meta;

type Story = StoryObj<typeof Table>;

export const Primary: Story = {
    args: {
        columns: mockTableColumns,
        rows: mockTableRows.slice(0, 5),
        className: "w-[600px]",
    },
};
export const Minimal: Story = {
    args: {
        columns: mockTableColumns,
        rows: mockTableRows.slice(0, 5),
        className: "w-full",
    },
};
export const Clip: Story = {
    render: (args) =>
        (
            <div className={cn("flex h-fit max-h-full w-full flex-col overflow-auto bg-gray-875", args.className)}>
                <Table {...args}/>
            </div>
        ),
    args: {
        columns: mockTableColumns,
        rows: mockTableRows,
        className: "w-96 h-96",
    },
};
