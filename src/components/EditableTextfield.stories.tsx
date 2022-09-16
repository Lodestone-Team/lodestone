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
};

export const Description = Template.bind({});
Description.args = {
  initialText: 'Hello World',
  type: 'description',
};
