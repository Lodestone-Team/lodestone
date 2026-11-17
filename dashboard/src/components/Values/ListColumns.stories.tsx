import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { Bytes, MonoText, VersionTag } from 'components/Atoms/Values';
import { Fact, FactLine } from './Fact';
import { ListCell, ListHeader } from './ListColumns';

const Row = ({ name, version, file, size, added }: Record<string, string>) => (
  <div className="flex items-center gap-3 border-b border-line py-1.5">
    <div className="flex min-w-0 grow flex-col">
      <div className="flex items-center gap-1.5 text-medium">
        <span className="truncate font-mediumbold text-gray-300">{name}</span>
        <VersionTag version={version} />
      </div>
      <FactLine>
        <MonoText text={file} max={30} />
        <Fact label="Size" className="@2xl:hidden">
          <Bytes bytes={Number(size)} />
        </Fact>
        <Fact label="Added" className="@2xl:hidden">
          {added}
        </Fact>
      </FactLine>
    </div>
    <ListCell width="size">
      <Bytes bytes={Number(size)} />
    </ListCell>
    <ListCell width="date">{added}</ListCell>
  </div>
);

const meta: Meta<typeof ListHeader> = {
  title: 'Values/ListColumns',
  component: ListHeader,
  render: () => (
    <div className="@container w-176 max-w-full resize-x overflow-hidden">
      <ListHeader
        columns={[
          { label: 'Size', width: 'size' },
          { label: 'Added', width: 'date' },
        ]}
      />
      <Row
        name="Fabric API"
        version="0.116.17+1.21.1"
        file="fabric-api-0.116.17+1.21.1.jar"
        size={String(2.5 * 1024 ** 2)}
        added="Nov 3, 2026"
      />
      <Row
        name="Sodium"
        version="mc1.21.1-0.6.0-fabric"
        file="sodium-fabric-0.6.0+mc1.21.1.jar"
        size={String(1.1 * 1024 ** 2)}
        added="Oct 30, 2026"
      />
    </div>
  ),
};
export default meta;

type Story = StoryObj<typeof ListHeader>;

/** Wide: Size and Added are columns */
export const Wide: Story = {};

/** Narrow: the columns fold back into each row's facts */
export const Narrow: Story = {
  decorators: [
    (Story) => (
      <div className="w-80">
        <Story />
      </div>
    ),
  ],
};
