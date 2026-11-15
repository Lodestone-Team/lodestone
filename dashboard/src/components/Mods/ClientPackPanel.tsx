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
import { useState } from 'react';
import { toast } from 'react-toastify';
import { installedModName, installedModVersion } from 'utils/mods';
import { errorToString } from 'utils/util';
import { packChips } from './clientPackFold';
import { ModIcon } from './common';

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
  const { t, formatList } = useI18n();
  const [downloading, setDownloading] = useState(false);
  // a big pack shows its first few mods, until "Show all"
  const [open, setOpen] = useState(false);
  if (list.loader !== 'fabric' || list.mods.length === 0) return null;

  const loader = list.loader_version
    ? t('mods.clientPack.loader', { version: list.loader_version })
    : t('mods.clientPack.loaderNoVersion');
  if (list.client_required_count === 0)
    return (
      <div className="text-medium text-white/50">
        {t('mods.clientPack.plainFabric', {
          gameVersion: list.game_version,
          loader,
        })}
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
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex flex-row items-center gap-2 text-h3 font-bold text-gray-300">
            <FontAwesomeIcon
              icon={faUsers}
              className="shrink-0 text-blue-200"
            />
            {t('mods.clientPack.title', { count })}
          </div>
          <div className="text-medium text-white/50">
            Minecraft {list.game_version} · {loader}
          </div>
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
          return (
            <div
              key={mod.file_name}
              className="flex max-w-full flex-row items-center gap-1.5 rounded-md bg-gray-900 py-1 pr-2 pl-1 text-small text-gray-300"
              title={
                mod.required_by.length > 0
                  ? t('mods.clientRequirement.neededBy', {
                      mods: formatList(mod.required_by),
                    })
                  : undefined
              }
            >
              <ModIcon url={mod.modrinth?.icon_url} className="size-5" />
              <span className="truncate">{installedModName(mod)}</span>
              {version && (
                <span className="shrink-0 text-white/50">{version}</span>
              )}
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
            : t('mods.clientPack.showAll', { count: needed.length })}
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
