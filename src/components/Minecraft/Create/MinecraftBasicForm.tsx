import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { MinecraftFlavour } from 'bindings/MinecraftFlavour';
import { MinecraftVersions } from 'bindings/MinecraftVersions';
import ComboField from 'components/Atoms/Form/ComboField';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';
import { useFormikContext } from 'formik';
import { MinecraftSetupConfigPrimitiveForm } from './form';

export default function MinecraftBasicForm() {
  const { data: minecraftFlavours, isLoading: minecraftFlavoursLoading } =
    useQuery<MinecraftFlavour[]>(['minecraft', 'flavours'], () =>
      axios.get('/games/minecraft/flavours').then((res) => {
        // sort by name
        return res.data.sort((a: MinecraftFlavour, b: MinecraftFlavour) => {
          return a.localeCompare(b);
        });
      })
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
            return [
              ...res.data.release,
              ...res.data.snapshot,
              ...res.data.old_alpha,
            ];
          }),
      { enabled: values.flavour !== '' }
    );
  return (
    <>
      <h1 className="text-larger font-bold tracking-tight text-gray-300">
        The Basics
      </h1>
      <p>
        Some basic information about your Minecraft server.
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
            values.flavour === ''
              ? 'Select a flavour first'
              : 'Select a version'
          }
          disabled={!values.flavour}
          loading={minecraftVersionsLoading}
          options={minecraftVersions ?? []}
          filterOptions={(query, options) => {
            return query === ''
              ? options
              : options.filter((option) =>
                  option.toLowerCase().startsWith(query.toLowerCase())
                );
          }}
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
