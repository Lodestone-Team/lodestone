import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { MonoText, MonoValue } from './MonoText';

const meta: Meta<typeof MonoText> = {
  title: 'Values/MonoText',
  component: MonoText,
  args: { className: 'text-small text-gray-400' },
};
export default meta;

type Story = StoryObj<typeof MonoText>;

/** A long file name keeps its extension */
export const FileName: Story = {
  args: {
    text: 'fabric-api-0.116.17+1.21.1-with-a-very-long-build-name.jar',
    max: 32,
  },
};

/** The home folder is "~" and the last two parts stay */
export const Path: Story = {
  args: {
    text: '/Users/ana/.lodestone/instances/a-modded-survival-world/mods',
    truncate: 'path',
    max: 32,
  },
};

export const Short: Story = { args: { text: 'mods/' } };

export const InSentence: Story = {
  render: () => (
    <p className="text-medium text-gray-300">
      Forward the port <MonoValue>25565</MonoValue> on your router.
    </p>
  ),
};
