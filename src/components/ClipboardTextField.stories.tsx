import { ComponentStory, ComponentMeta } from '@storybook/react';
import ClipboardTextField from './ClipboardTextField';

export default {
    title: 'library/ClickCopy',
    component: ClipboardTextField,
} as ComponentMeta<typeof ClipboardTextField>;

const Template: ComponentStory<typeof ClipboardTextField> = (args) => <ClipboardTextField {...args} />;

export const Default = Template.bind({});
Default.args = {
    text: 'Hello World',
    className: 'text-medium text-gray-300',
};
