import { register, init, getLocaleFromNavigator } from 'svelte-i18n';

register('af', () => import('$locales/af.json'));
register('am', () => import('$locales/am.json'));
register('ar', () => import('$locales/ar.json'));
register('az', () => import('$locales/az.json'));
register('be', () => import('$locales/be.json'));
register('bg', () => import('$locales/bg.json'));
register('bn', () => import('$locales/bn.json'));
register('ca', () => import('$locales/ca.json'));
register('cs', () => import('$locales/cs.json'));
register('da', () => import('$locales/da.json'));
register('de', () => import('$locales/de.json'));
register('el', () => import('$locales/el.json'));
register('en', () => import('$locales/en.json'));
register('es', () => import('$locales/es.json'));
register('et', () => import('$locales/et.json'));
register('eu', () => import('$locales/eu.json'));
register('fa', () => import('$locales/fa.json'));
register('fa-AE', () => import('$locales/fa-AE.json'));
register('fa-AF', () => import('$locales/fa-AF.json'));
register('fa-IR', () => import('$locales/fa-IR.json'));
register('fi', () => import('$locales/fi.json'));
register('fil', () => import('$locales/fil.json'));
register('fr', () => import('$locales/fr.json'));
register('gl', () => import('$locales/gl.json'));
register('gu', () => import('$locales/gu.json'));
register('he', () => import('$locales/he.json'));
register('hi', () => import('$locales/hi.json'));
register('hr', () => import('$locales/hr.json'));
register('hu', () => import('$locales/hu.json'));
register('hy', () => import('$locales/hy.json'));
register('id', () => import('$locales/id.json'));
register('is', () => import('$locales/is.json'));
register('it', () => import('$locales/it.json'));
register('ja', () => import('$locales/ja.json'));
register('ka', () => import('$locales/ka.json'));
register('kk', () => import('$locales/kk.json'));
register('km', () => import('$locales/km.json'));
register('kn', () => import('$locales/kn.json'));
register('ko', () => import('$locales/ko.json'));
register('ky', () => import('$locales/ky.json'));
register('lo', () => import('$locales/lo.json'));
register('lt', () => import('$locales/lt.json'));
register('lv', () => import('$locales/lv.json'));
register('mk', () => import('$locales/mk.json'));
register('ml', () => import('$locales/ml.json'));
register('mn', () => import('$locales/mn.json'));
register('mr', () => import('$locales/mr.json'));
register('ms', () => import('$locales/ms.json'));
register('my', () => import('$locales/my.json'));
register('nb', () => import('$locales/nb.json'));
register('ne', () => import('$locales/ne.json'));
register('nl', () => import('$locales/nl.json'));
register('nn', () => import('$locales/nn.json'));
register('pa', () => import('$locales/pa.json'));
register('pl', () => import('$locales/pl.json'));
register('pt', () => import('$locales/pt.json'));
register('rm', () => import('$locales/rm.json'));
register('ro', () => import('$locales/ro.json'));
register('ru', () => import('$locales/ru.json'));
register('si', () => import('$locales/si.json'));
register('sk', () => import('$locales/sk.json'));
register('sl', () => import('$locales/sl.json'));
register('sq', () => import('$locales/sq.json'));
register('sr', () => import('$locales/sr.json'));
register('sv', () => import('$locales/sv.json'));
register('sw', () => import('$locales/sw.json'));
register('ta', () => import('$locales/ta.json'));
register('te', () => import('$locales/te.json'));
register('th', () => import('$locales/th.json'));
register('tr', () => import('$locales/tr.json'));
register('uk', () => import('$locales/uk.json'));
register('ur', () => import('$locales/ur.json'));
register('vi', () => import('$locales/vi.json'));
register('zh-CN', () => import('$locales/zh-CN.json'));
register('zh-HK', () => import('$locales/zh-HK.json'));
register('zh-TW', () => import('$locales/zh-TW.json'));
register('zu', () => import('$locales/zu.json'));

export const LANGUAGES: { code: string; name: string; english: string }[] = [
  { code: 'af', name: 'Afrikaans', english: 'Afrikaans' },
  { code: 'sq', name: 'Shqip', english: 'Albanian' },
  { code: 'am', name: 'አማርኛ', english: 'Amharic' },
  { code: 'ar', name: 'العربية', english: 'Arabic' },
  { code: 'hy', name: 'Հայերեն', english: 'Armenian' },
  { code: 'az', name: 'Azərbaycanca', english: 'Azerbaijani' },
  { code: 'eu', name: 'Euskara', english: 'Basque' },
  { code: 'be', name: 'Беларуская', english: 'Belarusian' },
  { code: 'bn', name: 'বাংলা', english: 'Bengali' },
  { code: 'bg', name: 'Български', english: 'Bulgarian' },
  { code: 'my', name: 'မြန်မာ', english: 'Burmese' },
  { code: 'ca', name: 'Català', english: 'Catalan' },
  { code: 'zh-CN', name: '中文 (简体)', english: 'Chinese (Simplified)' },
  { code: 'zh-HK', name: '中文 (香港)', english: 'Chinese (Hong Kong)' },
  { code: 'zh-TW', name: '中文 (繁體)', english: 'Chinese (Traditional)' },
  { code: 'hr', name: 'Hrvatski', english: 'Croatian' },
  { code: 'cs', name: 'Čeština', english: 'Czech' },
  { code: 'da', name: 'Dansk', english: 'Danish' },
  { code: 'fa-AF', name: 'دری', english: 'Dari' },
  { code: 'nl', name: 'Nederlands', english: 'Dutch' },
  { code: 'en', name: 'English', english: 'English' },
  { code: 'et', name: 'Eesti', english: 'Estonian' },
  { code: 'fil', name: 'Filipino', english: 'Filipino' },
  { code: 'fi', name: 'Suomi', english: 'Finnish' },
  { code: 'fr', name: 'Français', english: 'French' },
  { code: 'gl', name: 'Galego', english: 'Galician' },
  { code: 'ka', name: 'ქართული', english: 'Georgian' },
  { code: 'de', name: 'Deutsch', english: 'German' },
  { code: 'el', name: 'Ελληνικά', english: 'Greek' },
  { code: 'gu', name: 'ગુજરાતી', english: 'Gujarati' },
  { code: 'he', name: 'עברית', english: 'Hebrew' },
  { code: 'hi', name: 'हिन्दी', english: 'Hindi' },
  { code: 'hu', name: 'Magyar', english: 'Hungarian' },
  { code: 'is', name: 'Íslenska', english: 'Icelandic' },
  { code: 'id', name: 'Bahasa Indonesia', english: 'Indonesian' },
  { code: 'it', name: 'Italiano', english: 'Italian' },
  { code: 'ja', name: '日本語', english: 'Japanese' },
  { code: 'kn', name: 'ಕನ್ನಡ', english: 'Kannada' },
  { code: 'kk', name: 'Қазақша', english: 'Kazakh' },
  { code: 'km', name: 'ភាសាខ្មែរ', english: 'Khmer' },
  { code: 'ko', name: '한국어', english: 'Korean' },
  { code: 'ky', name: 'Кыргызча', english: 'Kyrgyz' },
  { code: 'lo', name: 'ລາວ', english: 'Lao' },
  { code: 'lv', name: 'Latviešu', english: 'Latvian' },
  { code: 'lt', name: 'Lietuvių', english: 'Lithuanian' },
  { code: 'mk', name: 'Македонски', english: 'Macedonian' },
  { code: 'ms', name: 'Bahasa Melayu', english: 'Malay' },
  { code: 'ml', name: 'മലയാളം', english: 'Malayalam' },
  { code: 'mr', name: 'मराठी', english: 'Marathi' },
  { code: 'mn', name: 'Монгол', english: 'Mongolian' },
  { code: 'ne', name: 'नेपाली', english: 'Nepali' },
  { code: 'nb', name: 'Norsk (bokmål)', english: 'Norwegian (Bokmål)' },
  { code: 'nn', name: 'Norsk (nynorsk)', english: 'Norwegian (Nynorsk)' },
  { code: 'fa', name: 'فارسی', english: 'Persian' },
  { code: 'fa-AE', name: 'فارسی (امارات)', english: 'Persian (UAE)' },
  { code: 'fa-IR', name: 'فارسی (ایران)', english: 'Persian (Iran)' },
  { code: 'pl', name: 'Polski', english: 'Polish' },
  { code: 'pt', name: 'Português', english: 'Portuguese' },
  { code: 'pa', name: 'ਪੰਜਾਬੀ', english: 'Punjabi' },
  { code: 'ro', name: 'Română', english: 'Romanian' },
  { code: 'rm', name: 'Rumantsch', english: 'Romansh' },
  { code: 'ru', name: 'Русский', english: 'Russian' },
  { code: 'sr', name: 'Српски', english: 'Serbian' },
  { code: 'si', name: 'සිංහල', english: 'Sinhala' },
  { code: 'sk', name: 'Slovenčina', english: 'Slovak' },
  { code: 'sl', name: 'Slovenščina', english: 'Slovenian' },
  { code: 'es', name: 'Español', english: 'Spanish' },
  { code: 'sw', name: 'Kiswahili', english: 'Swahili' },
  { code: 'sv', name: 'Svenska', english: 'Swedish' },
  { code: 'ta', name: 'தமிழ்', english: 'Tamil' },
  { code: 'te', name: 'తెలుగు', english: 'Telugu' },
  { code: 'th', name: 'ไทย', english: 'Thai' },
  { code: 'tr', name: 'Türkçe', english: 'Turkish' },
  { code: 'uk', name: 'Українська', english: 'Ukrainian' },
  { code: 'ur', name: 'اردو', english: 'Urdu' },
  { code: 'vi', name: 'Tiếng Việt', english: 'Vietnamese' },
  { code: 'zu', name: 'isiZulu', english: 'Zulu' },
];

const STORAGE_KEY = 'ghost-auth-ext-locale';

export function getSystemLocale(): string {
  return getLocaleFromNavigator()?.split('-')[0] ?? 'en';
}

export function hasStoredLocale(): boolean {
  return localStorage.getItem(STORAGE_KEY) !== null;
}

export function clearStoredLocale() {
  localStorage.removeItem(STORAGE_KEY);
}

export function initI18n() {
  const stored = localStorage.getItem(STORAGE_KEY);
  const system = getSystemLocale();

  init({
    fallbackLocale: 'en',
    initialLocale: stored ?? system,
  });
}
