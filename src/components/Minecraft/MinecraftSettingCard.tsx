import { useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import DashboardCard from 'components/DashboardCard';
import SettingField from 'components/SettingField';
import { InstanceContext } from 'data/InstanceContext';
import { useInstanceManifest } from 'data/InstanceManifest';
import { useContext } from 'react';

export default function MinecraftSettingCard() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const { data: manifest, isLoading } = useInstanceManifest(instance.uuid);
  const supportedOptions = manifest?.supported_operations
    ? manifest.supported_operations
    : [];
  const supportedSettings = manifest?.settings ? manifest.settings : [];
  const uuid = instance.uuid;

  // hand picked list of minecraft settings to be shown
  const settings = [
    ['white-list', 'Whitelist', 'toggle'],
    ['enable-command-block', 'Command Blocks', 'toggle'],
    ['allow-flight', 'Flight', 'toggle'],
    ['spawn-animals', 'Spawn Animals', 'toggle'],
    ['spawn-monsters', 'Spawn Monsters', 'toggle'],
    ['spawn-npcs', 'Spawn NPCs', 'toggle'],
    ['allow-nether', 'Allow Nether', 'toggle'],
    ['force-gamemode', 'Force Gamemode', 'toggle'],
    ['spawn-protection', 'Spawn Protection', 'number'],
    ['require-resource-pack', 'Require Resource Pack', 'toggle'],
    ['resource-pack', 'Resource Pack', 'text'],
    ['resource-pack-prompt', 'Resource Pack Prompt', 'toggle'],
  ];

  const availableSettings = settings.filter((setting) =>
    supportedSettings.includes(setting[0])
  );

  if (isLoading) {
    return <div>Loading...</div>;
    // TODO: show an unobtrusive loading screen, reduce UI flicker
  }

  return (
    <div className="flex flex-col gap-4 @4xl:flex-row">
      <div className="w-[28rem]">
        <h1 className="text-large font-black"> Advanced Settings </h1>
        <h2 className="text-base font-medium italic tracking-tight text-white/50">
          Most users should not need to change these settings.
        </h2>
      </div>
      <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {availableSettings.map((setting) => {
          return (
            <SettingField
              instance={instance}
              setting={setting[0]}
              label={setting[1]}
              key={setting[0]}
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              type={setting[2] as any}
            />
          );
        })}
      </div>
    </div>
  );
}
