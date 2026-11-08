import SelectBox from 'components/Atoms/Config/SelectBox';
import {
  LANGUAGE_NAMES,
  LANGUAGES,
  LanguagePreference,
  useI18n,
  useLanguagePreference,
} from 'i18n';
import { browserLanguage } from 'i18n/language';

const preferences: LanguagePreference[] = ['auto', ...LANGUAGES];

/**
 * The dashboard language. Like the theme, it is saved in this browser
 * (localStorage), not on the core. "Browser default" follows the browser's
 * languages, else English.
 */
export default function LanguageSetting() {
  const { t } = useI18n();
  const [preference, setPreference] = useLanguagePreference();

  const label = (option: string) =>
    option === 'auto'
      ? t('settings.language.auto', {
          language: LANGUAGE_NAMES[browserLanguage()],
        })
      : // language names are written in their own language
        LANGUAGE_NAMES[option as keyof typeof LANGUAGE_NAMES];

  return (
    <SelectBox
      label={t('settings.language.label')}
      description={t('settings.language.hint')}
      value={preference}
      options={preferences}
      optionLabel={label}
      optimistic
      onChange={async (value) => setPreference(value as LanguagePreference)}
    />
  );
}
