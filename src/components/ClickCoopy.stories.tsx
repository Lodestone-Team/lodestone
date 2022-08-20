import { ComponentStory, ComponentMeta } from '@storybook/react';
import ClickCopy from './ClickCopy';

export default {
    title: 'library/ClickCopy',
    component: ClickCopy,
} as ComponentMeta<typeof ClickCopy>;

const Template: ComponentStory<typeof ClickCopy> = (args) => <ClickCopy {...args} />;

export const Default = Template.bind({});
Default.args = {
    text: 'Hello World',
    className: 'text-medium text-gray-300',
};
