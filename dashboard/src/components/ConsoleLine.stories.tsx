import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { ConsoleLine } from './ConsoleLine';
import { ConsoleTheme, parseConsoleLine } from 'utils/consoleFormat';

const E = '\x1b';

// Sample output from the servers and tools the console shows
const sampleLines = [
  '[12:00:00] [Server thread/INFO]: Starting minecraft server version 1.21.4',
  "[12:00:01] [Server thread/WARN]: Can't keep up! Is the server overloaded?",
  '[12:00:02] [Server thread/ERROR]: Encountered an unexpected exception',
  '[12:00:03 WARN]: [ViaVersion] You are running an outdated version',
  `${E}[38;2;85;255;255m[12:00:04 INFO]: ${E}[0m${E}[32;1mDone (3.21s)! ${E}[0mFor help, type "help"`,
  `[12:00:05 INFO]: ${E}[33;1m[Essentials]${E}[0m ${E}[93mbright yellow${E}[0m ${E}[37mwhite${E}[0m ${E}[97mbright white${E}[0m`,
  '§6[Server] §eWelcome §a§lSteve§r§7 to the server! §cLow TPS §9blue §dpink §bAqua §8dark gray §0black §fwhite',
  '§x§f§f§8§0§0§0Hex orange§r and §kobfuscated§r text',
  `${E}[30mblack ${E}[31mred ${E}[32mgreen ${E}[33myellow ${E}[34mblue ${E}[35mmagenta ${E}[36mcyan ${E}[37mwhite${E}[0m`,
  `${E}[90mblack ${E}[91mred ${E}[92mgreen ${E}[93myellow ${E}[94mblue ${E}[95mmagenta ${E}[96mcyan ${E}[97mwhite${E}[0m`,
  `${E}[1mbold${E}[0m ${E}[2mdim${E}[0m ${E}[3mitalic${E}[0m ${E}[4munderline${E}[0m ${E}[9mstrike${E}[0m ${E}[7minverse${E}[0m ${E}[4;9mboth${E}[0m`,
  `${E}[41;97m ERROR ${E}[0m ${E}[43;30m WARN ${E}[0m ${E}[42;30m PASS ${E}[0m ${E}[44m info ${E}[0m ${E}[31;7m inverse red ${E}[0m`,
  Array.from({ length: 24 }, (_, i) => `${E}[38;5;${16 + i * 9}m■${E}[0m`).join(
    ''
  ) +
    ' ' +
    Array.from({ length: 24 }, (_, i) => `${E}[38;5;${232 + i}m■`).join('') +
    `${E}[0m`,
  `${E}[38;2;0;0;128mnavy ${E}[38;2;255;255;0myellow ${E}[38;2;255;255;255mwhite ${E}[38;2;20;20;20mnear black${E}[0m`,
  `${E}]0;Window title${E}\\${E}[2K${E}[1Gcursor moves and titles are removed${E}[?25h`,
  'Tom & Jerry &a &l (no & codes)',
];

const parsed = sampleLines.map(parseConsoleLine);

const Console = ({
  theme,
  colored,
}: {
  theme: ConsoleTheme;
  colored: boolean;
}) => (
  <div data-theme={theme} className="w-176">
    <ol className="font-light flex flex-col rounded-lg border border-gray-faded/30 bg-gray-900 py-3 font-mono text-small tracking-tight break-words whitespace-pre-wrap text-gray-300">
      {parsed.map((line, index) => (
        <ConsoleLine key={index} line={line} colored={colored} theme={theme} />
      ))}
    </ol>
  </div>
);

const meta: Meta<typeof Console> = {
  title: 'components/ConsoleLine',
  component: Console,
  args: { colored: true },
};
export default meta;

type Story = StoryObj<typeof Console>;

export const Dark: Story = { args: { theme: 'dark' } };
export const Light: Story = { args: { theme: 'light' } };
export const Plain: Story = { args: { theme: 'dark', colored: false } };
