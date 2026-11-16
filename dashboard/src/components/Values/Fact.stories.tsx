import { faDownload, faUser } from '@fortawesome/free-solid-svg-icons';
import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { Bytes, MonoText, Num, VersionTag } from 'components/Atoms/Values';
import { Fact, FactLine, factsTitle } from './Fact';

const meta: Meta<typeof Fact> = {
  title: 'Values/Fact',
  component: Fact,
  args: { className: 'text-small text-gray-400' },
};
export default meta;

type Story = StoryObj<typeof Fact>;

export const Keyed: Story = {
  args: { label: 'Size', children: <Bytes bytes={2.5 * 1024 ** 2} /> },
};

/** In tight rows an icon stands for the key; screen readers get the key */
export const WithIcon: Story = {
  args: {
    label: 'Downloads',
    icon: faDownload,
    title: 'Downloads: 1,234,567',
    children: <Num value="1.2M" />,
  },
};

/** Line 2 of a row: facts that don't fit drop off, the title lists them all */
export const Line: StoryObj<typeof FactLine> = {
  render: () => (
    <div className="w-72 rounded-md bg-gray-850 p-2">
      <div className="flex items-center gap-1.5 text-medium">
        <span className="font-mediumbold text-gray-300">Fabric API</span>
        <VersionTag version="0.116.17+1.21.1" />
      </div>
      <FactLine
        title={factsTitle([
          { label: 'File', value: 'fabric-api-0.116.17+1.21.1.jar' },
          { label: 'Author', value: 'modmuss50' },
          { label: 'Size', value: '2.5 MB' },
        ])}
      >
        <MonoText text="fabric-api-0.116.17+1.21.1.jar" max={24} />
        <Fact label="Author" icon={faUser}>
          modmuss50
        </Fact>
        <Fact label="Size">
          <Bytes bytes={2.5 * 1024 ** 2} />
        </Fact>
      </FactLine>
    </div>
  ),
};
