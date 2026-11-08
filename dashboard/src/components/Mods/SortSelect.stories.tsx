import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import { useEffect, useState } from 'react';
import { SortSelect } from './ModrinthSearch';

/** The sort dropdown above the Modrinth results, in a theme set on the root
 * (its options open in a portal, outside any wrapper). */
const Demo = ({ theme }: { theme: 'dark' | 'light' }) => {
  const [index, setIndex] = useState<ModrinthSearchIndex>('relevance');
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);
  return (
    <div className="flex w-160 flex-row items-center justify-end rounded-lg bg-gray-800 p-4">
      <SortSelect value={index} onChange={setIndex} />
    </div>
  );
};

const meta: Meta<typeof Demo> = {
  title: 'components/Mods/SortSelect',
  component: Demo,
};

export default meta;
type Story = StoryObj<typeof Demo>;

export const Dark: Story = { args: { theme: 'dark' } };
export const Light: Story = { args: { theme: 'light' } };
