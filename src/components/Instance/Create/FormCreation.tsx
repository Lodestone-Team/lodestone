import FormInputField from './SetupFormFields/FormInputField';
import FormRadioField from './SetupFormFields/FormRadioField';
import FormComboField from './SetupFormFields/FormComboField';
import { SectionManifest, SettingManifest } from './form';
import { toast } from 'react-toastify';
export const createForm = (section: SectionManifest) => {
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
      <h1 className="text-h2 font-bold tracking-medium text-gray-300">
        {section.name}
      </h1>
      <p>
        {section.description}
        <br />
      </p>
      <div className="mt-9 flex flex-col rounded-md border border-gray-faded/30 text-left child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {Object.keys(section['settings']).map((field: string, i: number) =>
          createField(section['settings'][field], i)
        )}
      </div>
    </>
  );
};
