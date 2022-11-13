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
    <DashboardCard>
      <h1 className="text-medium font-bold"> Game Settings </h1>
      <div className="grid w-full grid-cols-2 gap-x-8 gap-y-16 child:w-full lg:grid-cols-4">
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
    </DashboardCard>
  );
}
