// Stories file for Button component
import { faServer } from '@fortawesome/free-solid-svg-icons';
import { ComponentStory, ComponentMeta } from '@storybook/react';
import Button from './Button';

export default {
  title: 'library/Button',
  component: Button,
  argTypes: {
    disabled: { control: 'boolean' },
    onClick: { table: { disable: true } },
    className: { table: { disable: true } },
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

export const StartServer = Template.bind({});
StartServer.args = {
  label: 'Start Server',
  onClick: () => {
    console.log('Start Server');
  },
  icon: faServer,
};
