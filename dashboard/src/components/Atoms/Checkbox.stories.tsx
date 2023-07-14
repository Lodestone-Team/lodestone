import { ComponentStory, ComponentMeta } from '@storybook/react';
import Checkbox from './Checkbox';

export default {
  title: 'library/Checkbox',
  component: Checkbox,
  argTypes: {
    onChange: { action: 'onChange' },
  },
} as ComponentMeta<typeof Checkbox>;

const Template: ComponentStory<typeof Checkbox> = (args) => (
  <Checkbox {...args} />
);
export const Default = Template.bind({});
Default.args = {
  label: 'Checkbox',
  checked: false,
  disabled: false,
};
export const Checked = Template.bind({});
Checked.args = {
  label: 'Checkbox',
  checked: true,
  disabled: false,
};
export const Disabled = Template.bind({});
Disabled.args = {
  label: 'Checkbox',
  checked: false,
  disabled: true,
};
