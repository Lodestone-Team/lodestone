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
      <div className="text-left text-h2 font-extrabold leading-7 tracking-medium text-white">
        {name}
      </div>
      <div className="mb-4 text-left text-medium font-mediumbold italic leading-4 text-white/50">
        {description}
      </div>
      {children}
      {Object.values(manifest['setting_sections']).map(
        (section, i: number) => (
          <div
            key={i}
            className="mt-9 flex flex-col rounded-md border border-gray-faded/30 text-left child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0"
          >
            {Object.keys(section['settings']).map((field: string) => (
              <FieldFromManifest
                setting={section['settings'][field]}
                key={field}
              />
            ))}
          </div>
        )
      )}
    </>
  );
};
