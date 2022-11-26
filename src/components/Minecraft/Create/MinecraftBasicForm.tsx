import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { MinecraftFlavour } from 'bindings/MinecraftFlavour';
import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import { MinecraftVersions } from 'bindings/MinecraftVersions';
import ComboField from 'components/Atoms/Form/ComboField';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';
import SelectField from 'components/Atoms/Form/SelectField';
import { LodestoneContext } from 'data/LodestoneContext';
import { Field, useFormikContext } from 'formik';
import { useContext } from 'react';
import { MinecraftSetupConfigPrimitiveForm } from './form';

export default function MinecraftBasicForm() {
  const { isReady } = useContext(LodestoneContext);
  const { data: minecraftFlavours, isLoading: minecraftFlavoursLoading } =
    useQuery<MinecraftFlavour[]>(
      ['minecraft', 'flavours'],
      () =>
        axios.get('/games/minecraft/flavours').then((res) => {
          // sort by name
          return res.data.sort((a: MinecraftFlavour, b: MinecraftFlavour) => {
            return a.localeCompare(b);
          });
        }),
      { enabled: isReady }
    );

  const { values } = useFormikContext<MinecraftSetupConfigPrimitiveForm>();

  const { data: minecraftVersions, isLoading: minecraftVersionsLoading } =
    useQuery<Array<string>>(
      ['minecraft', 'versions', values.flavour],
      () =>
        axios
          .get<MinecraftVersions>(
            `/games/minecraft/flavours/${values.flavour}/versions`
          )
          .then((res) => {
            return [...res.data.release, ...res.data.snapshot, ...res.data.old_alpha];
          }),
      { enabled: isReady && values.flavour !== '' }
    );
  return (
    <>
      <h1 className="text-larger font-bold tracking-tight text-gray-300">
        The Basics
      </h1>
      <p>
        Some basic information about your minecraft server.
        <br />
      </p>
      <div className="mt-10 flex flex-col gap-16 text-left">
        <RadioField
          name="flavour"
          label="Flavour"
          loading={minecraftFlavoursLoading}
          options={minecraftFlavours ?? []}
        />
        <ComboField
          name="version"
          label="Version"
          placeholder={
            values.flavour === '' ? 'Select a flavour first' : 'Select a version'
          }
          disabled={!values.flavour}
          loading={minecraftVersionsLoading}
          options={minecraftVersions ?? []}
        />
        <InputField
          type="number"
          name="port"
          label="Port"
          min={0}
          max={65535}
        />
      </div>
    </>
  );
}
