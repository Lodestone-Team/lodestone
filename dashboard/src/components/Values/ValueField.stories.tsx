import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { ValueField } from './ValueField';

const meta: Meta<typeof ValueField> = {
  title: 'Values/ValueField',
  component: ValueField,
  decorators: [
    (Story) => (
      <div className="w-80">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof ValueField>;

export const Path: Story = {
  args: {
    label: 'Folder',
    kind: 'path',
    value: '/Users/ana/.lodestone/instances/survival-smp-2026/world',
  },
};

export const Code: Story = {
  args: {
    label: 'UUID',
    kind: 'code',
    value: 'b6a1f0c2-7f1e-4b36-9e7a-2a9f6d0c8e11',
  },
};

export const Url: Story = {
  args: { label: 'Address', kind: 'url', value: 'https://example.com:16662' },
};

export const Secret: Story = {
  args: { kind: 'secret', value: 'lsk_live_0123456789abcdef' },
};
