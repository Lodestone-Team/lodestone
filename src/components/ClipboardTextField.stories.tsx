import { ComponentStory, ComponentMeta } from '@storybook/react';
import ClipboardTextField from './ClipboardTextField';

export default {
    title: 'library/ClipboardTextField',
    component: ClipboardTextField,
} as ComponentMeta<typeof ClipboardTextField>;

const Template: ComponentStory<typeof ClipboardTextField> = (args) => <ClipboardTextField {...args} />;

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
