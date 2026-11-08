import {
  faArrowUpRightFromSquare,
  faChevronDown,
  faChevronRight,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstanceInfo } from 'bindings/InstanceInfo';
import ClipboardTextfield from 'components/ClipboardTextfield';
import { game_to_game_title } from 'data/GameTypeMappings';
import { ReactNode, useState } from 'react';
import { safeLinkUrl } from 'utils/markdown';

// whether the section is open, remembered per browser
const OPEN_KEY = 'instanceDetailsOpen';

const readOpen = () => {
  try {
    return window.localStorage.getItem(OPEN_KEY) === 'true';
  } catch {
    return false;
  }
};

const writeOpen = (open: boolean) => {
  try {
    window.localStorage.setItem(OPEN_KEY, String(open));
  } catch {
    // storage blocked: the section just won't remember its state
  }
};

const yesNo = (value: boolean) => (value ? 'Yes' : 'No');

/** The loader name of a Minecraft Java instance, or null for vanilla. */
const loaderName = (instance: InstanceInfo) => {
  if (instance.game_type.type !== 'MinecraftJava') return null;
  const variant = instance.game_type.variant;
  switch (variant.type) {
    case 'Vanilla':
      return null;
    case 'Other':
      return variant.name;
    default:
      return variant.type;
  }
};

const formatCreated = (seconds: InstanceInfo['creation_time']) => {
  const date = new Date(Number(seconds) * 1000);
  if (Number(seconds) <= 0 || isNaN(date.getTime())) return null;
  return date.toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
};

const ModpackValue = ({
  modpack,
}: {
  modpack: NonNullable<InstanceInfo['modpack']>;
}) => {
  const label = modpack.version_number
    ? `${modpack.name} ${modpack.version_number}`
    : modpack.name;
  const href = modpack.project_id
    ? safeLinkUrl(
        `https://modrinth.com/modpack/${encodeURIComponent(modpack.project_id)}`
      )
    : null;
  if (!href) return <>{label}</>;
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer noopener"
      className="inline-flex items-center gap-1 text-blue-200 hover:underline"
    >
      {label}
      <FontAwesomeIcon icon={faArrowUpRightFromSquare} className="text-small" />
    </a>
  );
};

/**
 * A collapsible list of everything known about the instance, collapsed by
 * default (CLIENT_MODS_SPEC 3.2). Rows without a value are left out.
 */
export default function InstanceDetails({
  instance,
}: {
  instance: InstanceInfo;
}) {
  const [open, setOpen] = useState(readOpen);
  const toggle = () => {
    setOpen(!open);
    writeOpen(!open);
  };

  const isMinecraft =
    instance.game_type.type === 'MinecraftJava' ||
    instance.game_type.type === 'MinecraftBedrock';
  const loader = loaderName(instance);
  const created = formatCreated(instance.creation_time);
  const hasPlayers =
    instance.player_count !== null || instance.max_player_count !== null;
  const ram =
    instance.min_ram !== null || instance.max_ram !== null
      ? `${instance.min_ram ?? '?'} MB / ${instance.max_ram ?? '?'} MB`
      : null;

  const rows: Array<[string, ReactNode | null]> = [
    ['Name', instance.name],
    ['Description', instance.description || null],
    [
      'UUID',
      <ClipboardTextfield
        key="uuid"
        text={instance.uuid}
        iconLeft={false}
        className="justify-start! font-mono text-small"
      />,
    ],
    ['Game', game_to_game_title(instance.game_type)],
    [isMinecraft ? 'Minecraft version' : 'Version', instance.version || null],
    [
      'Loader',
      loader
        ? `${loader}${instance.loader_version ? ` ${instance.loader_version}` : ''}`
        : null,
    ],
    ['Port', String(instance.port)],
    ['State', instance.state],
    [
      'Players',
      hasPlayers
        ? `${instance.player_count ?? '?'} / ${instance.max_player_count ?? '?'}`
        : null,
    ],
    ['Created', created],
    ['Auto start', yesNo(instance.auto_start)],
    ['Restart on crash', yesNo(instance.restart_on_crash)],
    ['Min / Max RAM', ram],
    [
      'Folder',
      <ClipboardTextfield
        key="path"
        text={instance.path}
        iconLeft={false}
        className="justify-start! font-mono text-small break-all whitespace-normal!"
      />,
    ],
    [
      'Created from modpack',
      instance.modpack ? <ModpackValue modpack={instance.modpack} /> : null,
    ],
  ];

  return (
    <div className="flex w-full flex-col gap-2">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={open}
        className="flex flex-row items-center gap-2 self-start rounded-sm text-h3 font-bold text-gray-300 hover:text-white focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden"
      >
        <FontAwesomeIcon
          icon={open ? faChevronDown : faChevronRight}
          className="w-3 text-small text-white/50"
        />
        Details
      </button>
      {open && (
        <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-6 gap-y-2 rounded-lg border border-gray-faded/30 bg-gray-800 p-4 text-medium">
          {rows
            .filter(([, value]) => value !== null)
            .map(([label, value]) => (
              <div key={label} className="contents">
                <dt className="text-white/50">{label}</dt>
                <dd className="min-w-0 wrap-break-word text-gray-300">
                  {value}
                </dd>
              </div>
            ))}
        </dl>
      )}
    </div>
  );
}
