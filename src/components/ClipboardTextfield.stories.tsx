import { ComponentStory, ComponentMeta } from '@storybook/react';
import ClipboardTextfield from './ClipboardTextfield';

export default {
    title: 'library/ClickCopy',
    component: ClipboardTextField,
} as ComponentMeta<typeof ClipboardTextField>;

const Template: ComponentStory<typeof ClipboardTextfield> = (args) => <ClipboardTextfield {...args} />;

export const Default = Template.bind({});
Default.args = {
    text: 'Hello World',
    className: 'text-medium text-gray-300',
};

export const Small = Template.bind({});
Small.args = {
    text: 'Hello World',
    className: 'text-regular text-gray-300',
};

export const OverwriteIconStyle = Template.bind({});
OverwriteIconStyle.args = {
    text: 'Hello World',
    className: 'text-medium text-gray-300',
    iconClassName: 'text-red',
};
