import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import EditableTextfield from './EditableTextfield';

const meta: Meta<typeof EditableTextfield> = {
  title: 'library/EditableTextfield',
  component: EditableTextfield,
};
export default meta;

type Story = StoryObj<typeof EditableTextfield>;

export const Heading: Story = {
  args: {
    initialText: 'Hello World',
    onSubmit: async () => {},
  },
};

export const Description: Story = {
  args: {
    initialText: 'Hello World',
    type: 'description',
    onSubmit: async () => {},
  },
};

export const ErrorHeading: Story = {
  args: {
    initialText: 'Placeholder',
    type: 'heading',
    onSubmit: async () => {
      throw new Error('Test Error');
    },
  },
};

export const ErrorDescription: Story = {
  args: {
    initialText: 'Placeholder',
    type: 'description',
    onSubmit: async () => {
      throw new Error('Test Error');
    },
  },
};
