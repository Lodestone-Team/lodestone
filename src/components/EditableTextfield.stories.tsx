import { ComponentStory, ComponentMeta } from '@storybook/react';
import EditableTextfield from './EditableTextfield';

export default {
  title: 'library/EditableTextfield',
  component: EditableTextfield,
} as ComponentMeta<typeof EditableTextfield>;

const Template: ComponentStory<typeof EditableTextfield> = (args) => (
  <EditableTextfield {...args} />
);

export const Heading = Template.bind({});
Heading.args = {
  initialText: 'Hello World',
  onSubmit: () => {
    return { status: 0, message: 'Success' };
  },
};

export const Description = Template.bind({});
Description.args = {
  initialText: 'Hello World',
  type: 'description',
  onSubmit: () => {
    return { status: 0, message: 'Success' };
  },
};

export const ErrorTest = Template.bind({});
ErrorTest.args = {
  initialText: 'Placeholder',
  type: 'description',
  onSubmit: () => {
    return { status: 1, message: 'Mock Error' };
  },
};
