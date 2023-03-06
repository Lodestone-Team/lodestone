import ComboField from 'components/Atoms/Form/ComboField';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';
import { SectionManifest, SettingManifest } from './form';
import { toast } from 'react-toastify';
export const createForm = (section: SectionManifest) => {
  const createField = (setting: SettingManifest) => {
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
      default:
        toast.error('Error generating form: Unknown value type.');
        return <></>;
    }
  };
  return (
    <>
      <h1 className="text-h2 font-bold tracking-medium text-gray-300">
        {section.name}
      </h1>
      <p>
        {section.description}
        <br />
      </p>
      <div className="flex flex-col gap-14 pt-9 text-left">
        {Object.keys(section['settings']).map((field: string) =>
          createField(section['settings'][field])
        )}
      </div>
    </>
  );
};
