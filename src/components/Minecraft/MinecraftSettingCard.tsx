import { useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import DashboardCard from 'components/DashboardCard';
import SettingTextfield from 'components/SettingTextfield';
import { useInstanceManifest } from 'data/InstanceManifest';

export default function MinecraftSettingCard({
  instance,
}: {
  instance: InstanceInfo;
}) {
  const { data: manifest, isLoading } = useInstanceManifest(instance.uuid);
  const supportedOptions = manifest?.supported_operations
    ? manifest.supported_operations
    : [];
  const currentSettings = manifest?.settings ? manifest.settings : [];
  const uuid = instance.uuid;

  if (isLoading) {
    return <div>Loading...</div>;
    // TODO: show an unobtrusive loading screen, reduce UI flicker
  }

  return (
    <DashboardCard>
      <h1 className="font-bold text-medium"> Game Settings </h1>
      <div className="grid w-full grid-cols-2 gap-4 child:w-full md:grid-cols-4">
        {currentSettings.map((setting) => {
          return (
            <SettingTextfield
              instance={instance}
              settingName={setting}
              label={setting}
              key={setting}
            />
          );
        })}
      </div>
    </DashboardCard>
  );
}
