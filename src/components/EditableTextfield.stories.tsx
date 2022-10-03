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
  onSubmit: async () => {
    return { error: false, message: 'Success' };
  },
};

export const Description = Template.bind({});
Description.args = {
  initialText: 'Hello World',
  type: 'description',
  onSubmit: async () => {
    return { error: false, message: 'Success' };
  },
};

export const ErrorHeading = Template.bind({});
ErrorHeading.args = {
  initialText: 'Placeholder',
  type: 'heading',
  onSubmit: async () => {
    return { error: true, message: 'Mock Error' };
  },
};


export const ErrorDescription = Template.bind({});
ErrorDescription.args = {
  initialText: 'Placeholder',
  type: 'description',
  onSubmit: async () => {
    return { error: true, message: 'Mock Error' };
  },
};
