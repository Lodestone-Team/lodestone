import { HandlerGameType } from 'bindings/HandlerGameType';
import ComboField from 'components/Atoms/Form/ComboField';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';
import { SectionManifest, SectionManifestValue, SettingManifest } from './form';
import InputBox from 'components/Atoms/Config/InputBox';
export const createForm = (section: SectionManifest) => {
  const createField = (setting: SettingManifest) => {
    console.log(setting);
    switch (setting.value_type.type) {
      case 'String':
        return (
          <InputField
            type="text"
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
          />
        );
      case 'Integer':
      case 'UnsignedInteger':
      case 'Float':
        return (
          <InputField
            type="number"
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
          />
        );
      case 'Boolean':
        console.log(`${setting.setting_id}.value`);
        return (
          <RadioField
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
            options={['false', 'true']}
          />
        );
      case 'Enum':
        return (
          <ComboField
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
            options={setting.value_type.options ?? []}
            filterOptions={(query, options) => {
              return query === ''
                ? options
                : options.filter((option) =>
                    option.toLowerCase().startsWith(query.toLowerCase())
                  );
            }}
          />
        );
    }
  };
  return (
    <div className="w-full">
      <h1 className="text-h2 font-bold tracking-medium text-gray-300">
        {section.name}
      </h1>
      <p>
        {section.description}
        <br />
      </p>
      <div className="mt-10 flex flex-col gap-14 text-left">
        {Object.keys(section['settings']).map((field: string) =>
          createField(section['settings'][field])
        )}
      </div>
    </div>
  );
};
