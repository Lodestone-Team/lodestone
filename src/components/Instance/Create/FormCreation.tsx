import FormInputField from './SetupFormFields/FormInputField';
import FormRadioField from './SetupFormFields/FormRadioField';
import FormComboField from './SetupFormFields/FormComboField';
import { SectionManifest, SettingManifest } from './form';
import { toast } from 'react-toastify';
export const createForm = (section: SectionManifest) => {
  console.log(section);
  const createField = (setting: SettingManifest, index: number) => {
    switch (setting.value_type.type) {
      case 'String':
        return (
          <FormInputField
            key={index}
            type={setting.is_secret ? 'password' : 'text'}
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
            description={setting.description ?? ''}
            optional={setting.is_required ? false : true}
          />
        );
      case 'Integer':
      case 'UnsignedInteger':
      case 'Float':
        return (
          <FormInputField
            key={index}
            type="number"
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
            description={setting.description ?? ''}
            optional={setting.is_required ? false : true}
          />
        );
      case 'Boolean':
        return (
          <FormRadioField
            key={index}
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
            options={['false', 'true']}
            description={setting.description ?? ''}
            optional={setting.is_required ? false : true}
          />
        );
      case 'Enum':
        return (
          <FormComboField
            key={index}
            name={`${setting.setting_id}.value`}
            label={setting.name ?? ''}
            options={setting.value_type.options ?? []}
            description={setting.description ?? ''}
            optional={setting.is_required ? false : true}
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
      <div className="text-left text-h2 font-extrabold leading-7 tracking-medium text-white">
        {section.name}
      </div>
      <div className="text-left text-medium font-mediumbold italic leading-4 text-white/50">
        {section.description}
      </div>
      <div className="mt-9 flex flex-col rounded-md border border-gray-faded/30 text-left child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {Object.keys(section['settings']).map((field: string, i: number) =>
          createField(section['settings'][field], i)
        )}
      </div>
    </>
  );
};
