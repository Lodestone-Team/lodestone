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
    // 'gamemode',
    // 'difficulty',
    'white-list',
    // 'online-mode',
    // 'pvp',
    'enable-command-block',
    'allow-flight',
    'spawn-animals',
    'spawn-monsters',
    'spawn-npcs',
    'allow-nether',
    'force-gamemode',
    'spawn-protection',
    'require-resource-pack',
    'resource-pack',
    'resource-pack-prompt',
  ];

  const availableSettings = settings.filter((setting) =>
    supportedSettings.includes(setting)
  );

  if (isLoading) {
    return <div>Loading...</div>;
    // TODO: show an unobtrusive loading screen, reduce UI flicker
  }

  return (
    <div className="flex flex-col gap-4 @6xl:flex-row">
      <div className="w-96">
        <h1 className="font-black"> Advanced Settings </h1>
        <h2 className="font-medium text-base italic tracking-tight text-white/50">
          Most users should not need to change these settings.
        </h2>
      </div>
      <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {availableSettings.map((setting) => {
          return (
            <SettingField
              instance={instance}
              setting={setting}
              label={setting}
              key={setting}
            />
          );
        })}
      </div>
    </div>
  );
}
