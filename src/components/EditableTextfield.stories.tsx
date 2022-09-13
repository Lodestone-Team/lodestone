import { ComponentStory, ComponentMeta } from '@storybook/react';
import EditableTextfield from './EditableTextfield';

export default {
    title: 'library/EditableTextfield',
    component: EditableTextfield,
} as ComponentMeta<typeof EditableTextfield>;

const Template: ComponentStory<typeof EditableTextfield> = (args) => <EditableTextfield {...args} />;

export const Default = Template.bind({});
Default.args = {
    initialText: 'Hello World'
};
