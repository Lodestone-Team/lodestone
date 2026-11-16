import {
  faChevronDown,
  faChevronRight,
  faDownload,
  faUsers,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstalledModList } from 'bindings/InstalledModList';
import Button from 'components/Atoms/Button';
import { downloadClientPack } from 'data/Mods';
import { t as translate, useI18n } from 'i18n';
import { CountBadge, VersionTag } from 'components/Atoms/Values';
import { FactLine, FactStrip } from 'components/Values';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { installedModName, installedModVersion } from 'utils/mods';
import { errorToString } from 'utils/util';
import { gamePhrase, loaderPhrase } from 'utils/values';
import { packChips } from './clientPackFold';
import { ModIcon, NeededBy, WithTooltip } from './common';

/**
 * "Players need N mods to join" on a Fabric instance, with the mods and a
 * client modpack to download (CLIENT_MODS_SPEC 2.2). Shows one quiet line
 * when players need nothing but Fabric, and nothing on other loaders.
 */
export default function ClientPackPanel({
  uuid,
  list,
}: {
  uuid: string;
  list: InstalledModList;
}) {
  const { t } = useI18n();
  const [downloading, setDownloading] = useState(false);
  // a big pack shows its first few mods, until "Show all"
  const [open, setOpen] = useState(false);
  if (list.loader !== 'fabric' || list.mods.length === 0) return null;

  const game = gamePhrase(list.game_version);
  const loader = loaderPhrase('fabric', list.loader_version);
  if (list.client_required_count === 0)
    return (
      <div className="flex flex-row flex-wrap items-center gap-x-3 text-medium text-white/50">
        {t('mods.clientPack.plainFabric')}
        <FactLine className="text-medium text-gray-400">
          <span>{game}</span>
          <span>{loader}</span>
        </FactLine>
      </div>
    );

  // disabled mods never go into the pack
  const needed = list.mods.filter(
    (mod) => mod.enabled && mod.client_requirement === 'required'
  );
  // the core puts jars that aren't on Modrinth into the pack's overrides
  const includedJars = needed.filter((mod) => !mod.modrinth).length;
  const count = list.client_required_count;
  const { shown, folds } = packChips(needed.length, open);

  const download = async () => {
    setDownloading(true);
    try {
      await downloadClientPack(uuid);
    } catch (error) {
      toast.error(
        translate('mods.clientPack.downloadFailed', {
          error: errorToString(error),
        })
      );
    } finally {
      setDownloading(false);
    }
  };

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-blue-faded/50 bg-gray-800 p-4">
      <div className="flex flex-row flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-2">
          <div className="flex flex-row items-center gap-2 text-h3 font-bold text-gray-300">
            <FontAwesomeIcon
              icon={faUsers}
              className="shrink-0 text-blue-200"
            />
            {t('mods.clientPack.title')}
            <CountBadge count={count} />
          </div>
          <FactStrip
            items={[
              { label: t('values.key.game'), value: game },
              { label: t('values.key.loader'), value: loader },
            ]}
          />
        </div>
        <Button
          label={t('mods.clientPack.download')}
          icon={faDownload}
          intention="primary"
          loading={downloading}
          onClick={download}
        />
      </div>
      <div className="flex flex-row flex-wrap gap-2">
        {needed.slice(0, shown).map((mod) => {
          const version = installedModVersion(mod);
          const chip = (
            <div className="flex max-w-full flex-row items-center gap-1.5 rounded-md bg-gray-900 py-1 pr-2 pl-1 text-small text-gray-300">
              <ModIcon url={mod.modrinth?.icon_url} className="size-5" />
              <span className="truncate font-mediumbold">
                {installedModName(mod)}
              </span>
              {version && <VersionTag version={version} max={20} />}
            </div>
          );
          return mod.required_by.length > 0 ? (
            <WithTooltip
              key={mod.file_name}
              message={<NeededBy mods={mod.required_by} />}
            >
              {chip}
            </WithTooltip>
          ) : (
            <div key={mod.file_name} className="max-w-full">
              {chip}
            </div>
          );
        })}
      </div>
      {folds && (
        <button
          type="button"
          className="flex w-fit flex-row items-center gap-1 text-small text-blue-200 hover:underline"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          <FontAwesomeIcon
            icon={open ? faChevronDown : faChevronRight}
            className="size-2"
          />
          {open
            ? t('mods.clientPack.showFewer')
            : t('values.showMore', { count: needed.length - shown })}
        </button>
      )}
      <div className="text-small text-white/50">
        {includedJars > 0
          ? t('mods.clientPack.importHintWithJars', { count: includedJars })
          : t('mods.clientPack.importHint')}
      </div>
    </div>
  );
}
