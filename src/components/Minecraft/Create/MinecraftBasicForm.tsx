import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { MinecraftFlavour } from 'bindings/MinecraftFlavour';
import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import InputField from 'components/Atoms/Form/InputField';
import SelectField from 'components/Atoms/Form/SelectField';
import { LodestoneContext } from 'data/LodestoneContext';
import { Field, useFormikContext } from 'formik';
import { useContext } from 'react';

export default function MinecraftBasicForm() {
  const { isReady } = useContext(LodestoneContext);
  const { data: minecraftFlavours, isLoading: minecraftFlavoursLoading } =
    useQuery<MinecraftFlavour[]>(
      ['minecraft', 'flavours'],
      () => axios.get('/games/minecraft/flavours').then((res) => res.data),
      { enabled: isReady }
    );

  const { values } = useFormikContext<MinecraftSetupConfigPrimitive>();

  const { data: minecraftVersions, isLoading: minecraftVersionsLoading } =
    useQuery<string[]>(
      ['minecraft', 'versions', values.flavour],
      () =>
        axios
          .get(`/games/minecraft/flavours/${values.flavour}/versions`)
          .then((res) => res.data),
      { enabled: isReady && !!values.flavour }
    );

  return (
    <>
      <h1 className="font-bold tracking-tight text-gray-300 text-larger">
        The Basics
      </h1>
      <p>
        Some basic information about your minecraft server. You can change these
        at any time.
      </p>
      <div className="flex flex-col gap-12 mt-10 text-left">
        <SelectField
          name="flavour"
          label="Flavour"
          disabled={minecraftFlavoursLoading}
          options={minecraftFlavours ?? []}
        />
        <SelectField
          name="version"
          label="Version"
          disabled={minecraftVersionsLoading || !values.flavour}
          options={minecraftVersions ?? []}
        />
        <InputField type="number" name="port" label="Port" />
      </div>
    </>
  );
}
