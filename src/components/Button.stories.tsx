// Stories file for Button component
import { ComponentStory, ComponentMeta } from '@storybook/react';
import Button from './Button';

export default {
    title: 'Library/Button',
    component: Button,
    argTypes: {
        disabled: { control: 'boolean' },
        className: { control: null },
    },
} as ComponentMeta<typeof Button>;

const Template: ComponentStory<typeof Button> = (args) => <Button {...args} />;

export const Default = Template.bind({});
Default.args = {
    label: 'Button',
};

export const Disabled = Template.bind({});
Disabled.args = {
    label: 'Button',
    disabled: true,
};
