import { ComponentStory, ComponentMeta } from '@storybook/react';
import Label from './Label';

export default {
  title: 'Library/Label',
  component: Label,
  argTypes: {
    color: { control: 'inline-radio' },
    size: { control: 'inline-radio' },
    children: { control: 'text' },
  },
} as ComponentMeta<typeof Label>;

const Template: ComponentStory<typeof Label> = (args) => <Label {...args} />;

export const RunningCard = Template.bind({});
RunningCard.args = {
  children: 'Running',
  color: 'green',
};

export const CrashedPage = Template.bind({});
CrashedPage.args = {
  children: 'Crashed',
  color: 'red',
  size: 'large',
};
