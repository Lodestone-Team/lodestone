import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { MinecraftFlavour } from 'bindings/MinecraftFlavour';
import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import Button from 'components/Atoms/Button';
import Dropdown from 'components/Atoms/Dropdown';
import Textfield from 'components/Atoms/Textfield';
import { LodestoneContext } from 'data/LodestoneContext';
import { useClientInfo } from 'data/SystemInfo';
import { useContext, useState } from 'react';
import { axiosWrapper, parseintStrict } from 'utils/util';
import DashboardCard from '../DashboardCard';

export default function CreateMinecraftInstance({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [step, setStep] = useState(1);
  const { isReady } = useContext(LodestoneContext);
  const [versionError, setVersionError] = useState('');
  const [formData, setFormData] = useState<MinecraftSetupConfigPrimitive>({
    name: '',
    version: '',
    flavour: 'vanilla' as MinecraftFlavour,
    port: 25565,
    cmd_args: null,
    description: null,
    fabric_loader_version: null,
    fabric_installer_version: null,
    min_ram: null,
    max_ram: null,
    auto_start: null,
    restart_on_crash: null,
    timeout_last_left: null,
    timeout_no_activity: null,
    start_on_connection: null,
    backup_period: null,
  });
  const { data: minecraftFlavours, isLoading: minecraftFlavoursLoading } =
    useQuery<MinecraftFlavour[]>(
      ['minecraft', 'flavours'],
      () => axios.get('/games/minecraft/flavours').then((res) => res.data),
      { enabled: isReady }
    );
  const { data: minecraftVersions, isLoading: minecraftVersionsLoading } =
    useQuery<string[]>(
      ['minecraft', 'versions', formData.flavour],
      () =>
        axios
          .get(`/games/minecraft/flavours/${formData.flavour}/versions`)
          .then((res) => res.data),
      { enabled: isReady }
    );

  const createInstance = async () => {
    await axiosWrapper<void>({
      method: 'post',
      url: '/instance/minecraft',
      data: formData,
    });
  };

  const tryProceed = async () => {
    if (step === 1) {
      if (formData.name === '') throw new Error('Name cannot be empty');
      setStep(2);
    }
    if (step === 2) {
      if (formData.version === '')
        return setVersionError('Version cannot be empty');
      createInstance().then(onComplete);
    }
  };

  return (
    <div className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-900 px-12 py-24">
      <div className="flex flex-col items-center gap-6 text-center">
        {step === 1 && (
          <>
            <h1 className="font-bold tracking-tight text-gray-300 text-larger">
              Create an Instance
            </h1>
            <p>
              Create a new Minecraft server instance to play with your friends.
            </p>
            <div className="flex flex-row w-full gap-4">
              <Textfield
                label="Instance Name"
                className="text-left grow"
                onSubmit={tryProceed}
                value={formData.name}
                onChange={async (name) => {
                  setFormData({ ...formData, name });
                }}
                id="minecraft-instance-name"
                required={true}
                showIcons={false}
              />
              <Button
                label="Continue"
                form="minecraft-instance-name"
                type="submit"
              />
            </div>
          </>
        )}
        {step === 2 && (
          <>
            <h1 className="font-bold tracking-tight text-gray-300 text-larger">
              The Basics
            </h1>
            <p>
              Some basic information about your minecraft server. You can change
              these at any time.
            </p>
            <div className="flex flex-col items-stretch w-full gap-16 text-left">
              <Dropdown
                label="Flavour"
                options={minecraftFlavours ?? []}
                disabled={minecraftFlavoursLoading}
                value={formData.flavour}
                onChange={async (flavour) => {
                  setFormData({
                    ...formData,
                    flavour: flavour as MinecraftFlavour,
                  });
                }}
              />
              <Dropdown
                label="Version"
                options={minecraftVersions ?? []}
                disabled={minecraftVersionsLoading}
                value={formData.version}
                onChange={async (version) => {
                  setVersionError('');
                  setFormData({ ...formData, version });
                }}
                error={versionError}
              />
              <Textfield
                label="Port"
                value={formData.port.toString()}
                min={0}
                max={65535}
                validate={async (port) => {
                  const numPort = parseintStrict(port);
                  const result = await axiosWrapper<boolean>({
                    method: 'get',
                    url: `/check/port/${numPort}`,
                  });
                  if (result) throw new Error('Port not available');
                }}
                onChange={async (port) => {
                  const portNum = parseintStrict(port);
                  setFormData({ ...formData, port: portNum });
                }}
                onSubmit={tryProceed}
                id="minecraft-instance-port"
              />

              <Button
                label="Continue"
                form="minecraft-instance-port"
                type="submit"
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
