import { faDownload, faUsers } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstalledModList } from 'bindings/InstalledModList';
import Button from 'components/Atoms/Button';
import { downloadClientPack } from 'data/Mods';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { installedModName, installedModVersion } from 'utils/mods';
import { errorToString } from 'utils/util';
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
  const [downloading, setDownloading] = useState(false);
  if (list.loader !== 'fabric' || list.mods.length === 0) return null;

  const loader = list.loader_version
    ? `Fabric loader ${list.loader_version}`
    : 'Fabric loader';
  if (list.client_required_count === 0)
    return (
      <div className="text-medium text-white/50">
        Players can join with plain Fabric (Minecraft {list.game_version},{' '}
        {loader}).
      </div>
    );

  // disabled mods never go into the pack
  const needed = list.mods.filter(
    (mod) => mod.enabled && mod.client_requirement === 'required'
  );
  // the core puts jars that aren't on Modrinth into the pack's overrides
  const includedJars = needed.filter((mod) => !mod.modrinth).length;
  const count = list.client_required_count;

  const download = async () => {
    setDownloading(true);
    try {
      await downloadClientPack(uuid);
    } catch (error) {
      toast.error(
        `Failed to download the client modpack: ${errorToString(error)}`
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
            <FontAwesomeIcon icon={faUsers} className="text-blue-200" />
            Players need {count} {count === 1 ? 'mod' : 'mods'} to join
          </div>
          <div className="text-medium text-white/50">
            Minecraft {list.game_version} · {loader}
          </div>
        </div>
        <Button
          label="Download client modpack (.mrpack)"
          icon={faDownload}
          intention="primary"
          loading={downloading}
          onClick={download}
        />
      </div>
      <div className="flex flex-row flex-wrap gap-2">
        {needed.map((mod) => {
          const version = installedModVersion(mod);
          return (
            <div
              key={mod.file_name}
              className="flex max-w-full flex-row items-center gap-1.5 rounded-md bg-gray-900 py-1 pr-2 pl-1 text-small text-gray-300"
              title={
                mod.required_by.length > 0
                  ? `Needed by ${mod.required_by.join(', ')}`
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
      <div className="text-small text-white/50">
        Import it into Prism Launcher, the Modrinth App or ATLauncher to get
        everything at the right versions.
        {includedJars > 0 &&
          ` ${includedJars} ${
            includedJars === 1 ? "mod isn't" : "mods aren't"
          } on Modrinth, so ${
            includedJars === 1 ? 'its file is' : 'their files are'
          } included in the pack.`}
      </div>
    </div>
  );
}
