import { ComponentStory, ComponentMeta } from '@storybook/react';
import ClipboardTextfield from './ClipboardTextfield';

export default {
    title: 'library/ClickCopy',
    component: ClipboardTextfield,
} as ComponentMeta<typeof ClipboardTextfield>;

const Template: ComponentStory<typeof ClipboardTextfield> = (args) => <ClipboardTextfield {...args} />;

export const Default = Template.bind({});
Default.args = {
    text: 'Hello World',
    className: 'text-medium text-gray-300',
};
