import { ComponentStory, ComponentMeta } from '@storybook/react';
import EditableTextfield from './EditableTextfield';
import { Result } from '@badrap/result';
import { ClientError } from 'data/ClientError';

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
  onSubmit: async () => Result.ok<void, ClientError>(undefined),
};

export const Description = Template.bind({});
Description.args = {
  initialText: 'Hello World',
  type: 'description',
  onSubmit: async () => Result.ok<void, ClientError>(undefined),
};

export const ErrorHeading = Template.bind({});
ErrorHeading.args = {
  initialText: 'Placeholder',
  type: 'heading',
  onSubmit: async () =>
    Result.err<ClientError, void>(
      new ClientError('UnknownError', 'Debug Error')
    ),
};

export const ErrorDescription = Template.bind({});
ErrorDescription.args = {
  initialText: 'Placeholder',
  type: 'description',
  onSubmit: async () =>
    Result.err<ClientError, void>(
      new ClientError('UnknownError', 'Debug Error')
    ),
};
