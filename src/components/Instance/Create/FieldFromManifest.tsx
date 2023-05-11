import { toast } from 'react-toastify';
import FormComboField from './SetupFormFields/FormComboField';
import FormInputField from './SetupFormFields/FormInputField';
import FormRadioField from './SetupFormFields/FormRadioField';
import { SettingManifest } from './form';

export const FieldFromManifest = ({
  setting,
}: {
  setting: SettingManifest;
}) => {
  switch (setting.value_type.type) {
    case 'String':
      return (
        <FormInputField
          key={setting.name}
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
          key={setting.name}
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
          key={setting.name}
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
          key={setting.name}
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
