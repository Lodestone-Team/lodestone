import {
  faArrowUpRightFromSquare,
  faChevronDown,
  faChevronRight,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { game_to_game_title } from 'data/GameTypeMappings';
import { ReactNode, useState } from 'react';
import { safeLinkUrl } from 'utils/markdown';
import { useI18n } from 'i18n';
import {
  Bytes,
  Fraction,
  Num,
  ObjectChip,
  VersionTag,
} from 'components/Atoms/Values';
import { FactList, ValueField } from 'components/Values';
import { instanceGamePhrase, instanceLoaderPhrase } from 'utils/values';

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

/** The creation time as a Date, or null if the core doesn't know it */
const createdDate = (seconds: InstanceInfo['creation_time']) => {
  const date = new Date(Number(seconds) * 1000);
  if (Number(seconds) <= 0 || isNaN(date.getTime())) return null;
  return date;
};

const ModpackValue = ({
  modpack,
}: {
  modpack: NonNullable<InstanceInfo['modpack']>;
}) => {
  const href = modpack.project_id
    ? safeLinkUrl(
        `https://modrinth.com/modpack/${encodeURIComponent(modpack.project_id)}`
      )
    : null;
  const version = modpack.version_number && (
    <VersionTag version={modpack.version_number} />
  );
  if (!href)
    return (
      <>
        <ObjectChip kind="modpack" name={modpack.name} />
        {version}
      </>
    );
  return (
    <>
      <a
        href={href}
        target="_blank"
        rel="noreferrer noopener"
        className="inline-flex min-w-0 items-center gap-1 text-blue-200 hover:underline"
      >
        <span className="truncate font-mediumbold">{modpack.name}</span>
        <FontAwesomeIcon
          icon={faArrowUpRightFromSquare}
          className="text-small"
        />
      </a>
      {version}
    </>
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
  const { t, formatDateTime } = useI18n();
  const yesNo = (value: boolean) => t(value ? 'common.yes' : 'common.no');
  const [open, setOpen] = useState(readOpen);
  const toggle = () => {
    setOpen(!open);
    writeOpen(!open);
  };

  const isMinecraft =
    instance.game_type.type === 'MinecraftJava' ||
    instance.game_type.type === 'MinecraftBedrock';
  const loader = instanceLoaderPhrase(
    instance.game_type,
    instance.loader_version
  );
  const createdAt = createdDate(instance.creation_time);
  const created = createdAt && formatDateTime(createdAt);
  const hasPlayers =
    instance.player_count !== null || instance.max_player_count !== null;
  const hasRam = instance.min_ram !== null || instance.max_ram !== null;
  // in MB, the unit in the language ("Mo" in French)
  const ram = (mb: number | null) =>
    mb === null ? (
      <Num value={null} />
    ) : (
      <Bytes bytes={mb * 1024 * 1024} decimals={0} unitOf={1024 * 1024} />
    );

  const rows: Array<[string, ReactNode | null]> = [
    [t('server.details.name'), instance.name],
    [t('server.details.description'), instance.description || null],
    [
      t('server.details.uuid'),
      <ValueField
        key="uuid"
        kind="code"
        value={instance.uuid}
        className="w-full max-w-sm"
      />,
    ],
    // Minecraft is one phrase with its version; other games keep a Version row
    [
      t('values.key.game'),
      isMinecraft
        ? instanceGamePhrase(instance.game_type, instance.version || null)
        : game_to_game_title(instance.game_type),
    ],
    [
      t('server.details.version'),
      isMinecraft ? null : instance.version || null,
    ],
    [t('values.key.loader'), loader],
    [
      t('server.details.port'),
      <Num key="port" value={String(instance.port)} />,
    ],
    [t('server.details.state'), t(`common.state.${instance.state}`)],
    [
      t('server.details.players'),
      hasPlayers ? (
        <Fraction
          key="players"
          done={instance.player_count}
          total={instance.max_player_count}
        />
      ) : null,
    ],
    [t('server.details.created'), created],
    [t('server.details.autoStart'), yesNo(instance.auto_start)],
    [t('server.details.restartOnCrash'), yesNo(instance.restart_on_crash)],
    [t('values.key.minRam'), hasRam ? ram(instance.min_ram) : null],
    [t('values.key.maxRam'), hasRam ? ram(instance.max_ram) : null],
    [
      t('server.details.folder'),
      <ValueField
        key="path"
        kind="path"
        value={instance.path}
        className="w-full max-w-sm"
      />,
    ],
    [
      t('values.key.modpack'),
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
        {t('server.details.title')}
      </button>
      {open && (
        <FactList
          framed
          className="py-2"
          rows={rows
            .filter(([, value]) => value !== null)
            .map(([label, value]) => ({ label, value }))}
        />
      )}
    </div>
  );
}
