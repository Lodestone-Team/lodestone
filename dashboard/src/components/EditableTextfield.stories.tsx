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
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  onSubmit: async () => {},
};

export const Description = Template.bind({});
Description.args = {
  initialText: 'Hello World',
  type: 'description',
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  onSubmit: async () => {},
};

export const ErrorHeading = Template.bind({});
ErrorHeading.args = {
  initialText: 'Placeholder',
  type: 'heading',
  onSubmit: async () => {
    throw new Error('Test Error');
  },
};

export const ErrorDescription = Template.bind({});
ErrorDescription.args = {
  initialText: 'Placeholder',
  type: 'description',
  onSubmit: async () => {
    throw new Error('Test Error');
  },
};
