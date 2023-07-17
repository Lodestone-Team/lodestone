import { MOTDRender } from 'components/Atoms/MOTDRender';
import { parse } from 'minecraft-motd-util';
import { convertUnicode } from 'utils/util';

export const SettingOverrides: Record<string, any> = {
  motd: {
    name: 'MOTD: Message of the Day',
    type: 'text',
    descriptionFunc: (motd: string) => (
      <div
        className={`mt-1 whitespace-pre-wrap p-2 font-minecraft text-medium text-[gray]`}
        style={{ backgroundImage: `url(/assets/dirt.png)` }}
      >
        <MOTDRender motd={parse(convertUnicode(motd))} />
      </div>
    ),
  },
};
