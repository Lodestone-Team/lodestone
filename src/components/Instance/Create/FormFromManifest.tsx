import FormInputField from './SetupFormFields/FormInputField';
import FormRadioField from './SetupFormFields/FormRadioField';
import FormComboField from './SetupFormFields/FormComboField';
import { SectionManifest, SettingManifest } from './form';
import { toast } from 'react-toastify';
import { FieldFromManifest } from './FieldFromManifest';

export const FormFromManifest = ({
  section,
  children,
}: {
  section: SectionManifest;
  children: React.ReactNode;
}) => {
  console.log("section", section)
  return (
    <>
      <div className="text-left text-h2 font-extrabold leading-7 tracking-medium text-white">
        {section.name}
      </div>
      <div className="text-left text-medium font-mediumbold italic leading-4 text-white/50 mb-4">
        {section.description}
      </div>
      {children}
      <div className="mt-5 flex flex-col rounded-md border border-gray-faded/30 text-left child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {Object.keys(section['settings']).map((field: string, i: number) => (
          <FieldFromManifest setting={section['settings'][field]} />
        ))}
      </div>
    </>
  );
};
