import { FieldFromManifest } from './FieldFromManifest';
import { SetupManifest } from 'bindings/SetupManifest';

export const FormFromManifest = ({
  name,
  description,
  manifest,
  children,
}: {
  name: string;
  description: string;
  manifest: SetupManifest;
  children: React.ReactNode;
}) => {
  return (
    <>
      <div className="text-left text-h2 leading-7 font-extrabold tracking-medium text-white">
        {name}
      </div>
      <div className="mb-4 text-left text-medium leading-4 font-mediumbold text-white/50 italic">
        {description}
      </div>
      {children}
      {Object.values(manifest['setting_sections']).map((section, i: number) => (
        <div
          key={i}
          className="mt-9 flex flex-col rounded-md border border-gray-faded/30 text-left child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0"
        >
          {Object.keys(section['settings']).map((field: string) => (
            <FieldFromManifest
              setting={section['settings'][field]}
              key={field}
            />
          ))}
        </div>
      ))}
    </>
  );
};
