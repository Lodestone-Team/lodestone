import {
  faBoxOpen,
  faCircleUser,
  faDatabase,
  faFile,
  faFileZipper,
  faFolder,
  faImages,
  faKey,
  faMicrochip,
  faPlug,
  faPuzzlePiece,
  faServer,
  faUser,
  faUserAstronaut,
  faWandMagicSparkles,
  IconDefinition,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useT } from 'i18n';
import { cn } from 'utils/util';

export type ObjectKind =
  | 'server'
  | 'mod'
  | 'plugin'
  | 'modpack'
  | 'file'
  | 'folder'
  | 'archive'
  | 'user'
  | 'account'
  | 'apiKey'
  | 'player'
  | 'core'
  | 'resourcepack'
  | 'shader'
  | 'datapack';

/** The icon of each kind of object, shared by chips, cards and lists */
export const KIND_ICONS: Record<ObjectKind, IconDefinition> = {
  server: faServer,
  mod: faPuzzlePiece,
  plugin: faPlug,
  modpack: faBoxOpen,
  file: faFile,
  folder: faFolder,
  archive: faFileZipper,
  user: faUser,
  account: faCircleUser,
  apiKey: faKey,
  player: faUserAstronaut,
  core: faMicrochip,
  resourcepack: faImages,
  shader: faWandMagicSparkles,
  datapack: faDatabase,
};

/**
 * The kind of an object as a small grey icon, instead of "(folder)". Screen
 * readers get the kind's name.
 */
export function KindIcon({
  kind,
  className,
}: {
  kind: ObjectKind;
  className?: string;
}) {
  const t = useT();
  return (
    <>
      <FontAwesomeIcon
        icon={KIND_ICONS[kind]}
        className={cn('w-3 shrink-0 text-gray-500', className)}
        aria-hidden="true"
      />
      <span className="sr-only">{t(`values.kind.${kind}`)} </span>
    </>
  );
}
