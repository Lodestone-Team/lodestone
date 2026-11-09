import Checkbox from 'components/Atoms/Checkbox';
import { MINECRAFT_EULA_URL } from 'data/MinecraftEula';
import { useI18n } from 'i18n';

/** A link to Mojang's Minecraft EULA, opened in a new tab. */
export function MinecraftEulaLink() {
  const { t } = useI18n();
  return (
    <a
      href={MINECRAFT_EULA_URL}
      target="_blank"
      rel="noreferrer"
      // the link sits in a checkbox's label: opening it must not tick the box
      onClick={(e) => e.stopPropagation()}
      className="text-blue-200 underline hover:text-blue-300"
    >
      {t('create.eula.link')}
    </a>
  );
}

/**
 * The required "I agree to the Minecraft EULA" box before creating a
 * Minecraft server. It starts unticked: only the user can agree.
 */
export default function EulaCheckbox({
  checked,
  onChange,
  disabled = false,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  const { t, rich } = useI18n();
  return (
    <div className="flex flex-col gap-1 text-left">
      <Checkbox
        label={rich('create.eula.agree', { link: <MinecraftEulaLink /> })}
        checked={checked}
        onChange={onChange}
        disabled={disabled}
      />
      {!checked && (
        <div className="text-small text-white/50">
          {t('create.eula.required')}
        </div>
      )}
    </div>
  );
}
