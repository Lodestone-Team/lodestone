import { ComponentStory, ComponentMeta } from '@storybook/react';
import GameIcon from './GameIcon';

export default {
  title: 'library/GameIcon',
  component: GameIcon,
} as ComponentMeta<typeof GameIcon>;

const Template: ComponentStory<typeof GameIcon> = (args) => (
  <GameIcon {...args} />
);
export const MinecraftVanilla = Template.bind({});
MinecraftVanilla.args = {
  game_type: 'minecraft',
  game_flavour: 'vanilla',
};
