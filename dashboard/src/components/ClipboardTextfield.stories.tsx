import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import ClipboardTextfield from './ClipboardTextfield';

const meta: Meta<typeof ClipboardTextfield> = {
  title: 'library/ClipboardTextfield',
  component: ClipboardTextfield,
};
export default meta;

type Story = StoryObj<typeof ClipboardTextfield>;

export const Default: Story = {
  args: {
    text: 'Hello World',
    className: 'text-h3 text-gray-300',
  },
};

export const Small: Story = {
  args: {
    text: 'Hello World',
    className: 'text-regular text-gray-300',
  },
};
