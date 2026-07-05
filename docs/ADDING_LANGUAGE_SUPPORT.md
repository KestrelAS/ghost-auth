# Ghost Auth: Internationalization & Translation Guide

## Overview

Ghost Auth supports 79 locales covering well over 95% of the world's native language speakers. This document provides guidelines for adding new locales, translating strings, and maintaining translation quality across all supported languages.

---

## Adding a New Locale

Adding a new language requires two steps:

1. Register the locale in `index.ts`:
   ```ts
   register('nb', () => import('./locales/nb.json'));
   ```

2. Add the language entry to the `LANGUAGES` array:
   ```ts
   { code: 'nb', name: 'Norsk' }
   ```

The `name` field should always be written in the language's **own script** (e.g., `Deutsch`, not `German`; `العربية`, not `Arabic`). This is what users see in the language picker.

---

## Locale Code Mapping

The planned locale list is derived from **Google Play Store** locale codes. These do **not** always match [BCP 47 / IETF](https://www.w3.org/International/questions/qa-choosing-language-tags) codes, which are used by browsers (`navigator.language`), the `Intl` API, and most i18n libraries including `svelte-i18n`.

When registering locales in the app, use **BCP 47 codes** as the canonical identifier. The app must handle mapping from Play Store codes where they differ.

### Known Mismatches

| Play Store Code | BCP 47 Code | Language | Notes |
|-----------------|-------------|----------|-------|
| `iw-IL` | `he-IL` | Hebrew | `iw` was deprecated in 1989; browsers return `he` |
| `no-NO` | `nb-NO` / `nn-NO` | Norwegian | Play Store uses generic `no`; in practice this maps to Bokmål (`nb`). Decide whether Nynorsk (`nn`) is also needed. |

### Redundant or Overlapping Codes

| Codes | Language | Recommendation |
|-------|----------|----------------|
| `ms` and `ms-MY` | Malay | These are effectively the same. Use `ms` as the base locale unless you have distinct content for Malaysia vs. other Malay-speaking regions. |
| `fa`, `fa-AE`, `fa-AF`, `fa-IR` | Persian / Dari | `fa` and `fa-IR` are functionally identical. `fa-AF` (Dari) has some vocabulary differences. Consider whether you need all four variants or can consolidate. |

### Browser Matching

Browsers send BCP 47 codes via `navigator.language` (e.g., `he-IL`, not `iw-IL`). When resolving the user's preferred locale, the app should:

1. Attempt an exact match against registered locales.
2. Fall back to the base language code (e.g., `he-IL` → `he`).
3. Fall back to the app's default locale (`en`).

Ensure the mapping layer accounts for the deprecated codes listed above if any platform still sends them.

---

## Translation Guidelines

### Tone and Formality

Ghost Auth is a security-sensitive application. Translations should use the **formal register** of the target language where a distinction exists:

- **German:** Use "Sie" (not "du")
- **French:** Use "vous" (not "tu")
- **Spanish:** Use "usted" (not "tú")
- **Portuguese (BR):** Use "você" (acceptable as neutral formal)
- **Japanese:** Use です/ます (desu/masu) polite form
- **Korean:** Use 합니다 (hapnida) formal polite form

When in doubt, err on the side of formality. Users trust security apps more when the tone is professional and respectful.

### Natural Language, Not Word-for-Word

Translations should read as **natural, fluent text** in the target language, not as a word-for-word transliteration from English. The use of symbols, abbreviations, and terminology should match the conventions of the translated language.

For example, the English string `"Scan QR code"` might become `"QR-kode scannen"` in Dutch but should **not** be `"Scan QR code"` left in English if the target language has natural equivalents for "scan."

### Placeholders and Variables

Translation strings may contain placeholders such as `{count}`, `{email}`, `{appName}`, or `{timeRemaining}`. Translators must:

- **Preserve all placeholders exactly**: do not translate, rename, or remove them.
- **Reorder freely**: word order differs between languages. Place the variable where it fits naturally in the target language's grammar.

Example:
```
English:  "{count} codes remaining"
Japanese: "残りのコード: {count}"
Arabic:   "{count} رموز متبقية"
```

### Pluralization

Languages handle plural forms very differently. The app uses ICU MessageFormat syntax for plurals. Translators should be aware that:

- **English** has 2 forms (singular, plural)
- **Arabic** has 6 forms (zero, one, two, few, many, other)
- **Polish** has 3 forms (one, few, many)
- **Japanese, Chinese, Korean, Vietnamese, Thai** have 1 form (no plural distinction)

Example ICU plural string:
```
{count, plural, one {# code remaining} other {# codes remaining}}
```

Translators must provide all plural forms required by their language.

### String Length and UI Constraints

Some languages expand significantly compared to English:

| Language | Typical Expansion |
|----------|-------------------|
| German | +30–35% |
| Finnish | +30–40% |
| French | +15–20% |
| Arabic | +20–25% |
| Japanese | Often shorter |
| Chinese | Often shorter |

Be aware of this when translating strings for buttons, labels, and other space-constrained UI elements. If a translation is significantly longer than the English source, try to find a concise alternative that preserves meaning.

### Right-to-Left (RTL) Languages

The following supported locales are **right-to-left**:

- `ar`: Arabic
- `he`: Hebrew (registered as `iw-IL` on Play Store)
- `ur`: Urdu
- `fa`, `fa-AE`, `fa-AF`, `fa-IR`: Persian / Dari

Translators for RTL languages should verify that:

- Text direction reads correctly in context.
- Punctuation, numbers, and bidirectional text (e.g., brand names in Latin script within an RTL sentence) display properly.
- UI mirroring (icons, navigation) is accounted for.

### Do Not Translate

The following terms should be kept in their original form across **all** languages:

- **Ghost Auth** (product name)
- **TOTP**, **HOTP** (protocol names)
- **QR** (as in QR code)
- **URI**, **URL**
- **SHA-1**, **SHA-256**, **SHA-512** (algorithm names)
- **Base32** (encoding format)
- **PIN** (where used as a technical term, though the concept may be translated)

If the target language commonly uses a transliteration of these terms (e.g., "КР-код" in Russian for "QR code"), the transliteration is acceptable alongside or in place of the Latin original, as long as users will understand it.

### Handling Uncertainty

If a translator is unsure about the best translation for a string:

- **Flag the string** by adding a comment in the JSON file or translation management system.
- **Do not leave it in English** without flagging. Silent untranslated strings are harder to catch than flagged ones.
- **Provide a best-effort translation** with a note explaining the uncertainty.

---

## Translation Process and Quality

### Who Translates?

Translations may come from professional translators, community contributors, or AI-assisted translation with human review. Regardless of the source, all translations must be:

- Reviewed by at least one native speaker.
- Tested in the actual UI to verify fit and context.
- Checked for placeholder integrity (no missing or broken `{variables}`).

### Quality Checklist

Before merging a new locale, verify:

- [ ] All keys from `en.json` are present in the new locale file.
- [ ] No placeholders are missing or malformed.
- [ ] Plural forms are complete for the target language.
- [ ] Formal register is used where applicable.
- [ ] RTL rendering works correctly (if applicable).
- [ ] String lengths don't break any UI elements.
- [ ] The locale is registered in `index.ts`.
- [ ] The language is added to the `LANGUAGES` array with its native name.

---

## Currently Supported Locales

- `af`: Afrikaans
- `am`: አማርኛ
- `ar`: العربية (RTL)
- `az`: Azərbaycanca
- `be`: Беларуская
- `bg`: Български
- `bn`: বাংলা
- `ca`: Català
- `cs`: Čeština
- `da`: Dansk
- `de`: Deutsch
- `el`: Ελληνικά
- `en`: English (source language, fallback; base locale for all English variants)
- `es`: Español (base locale for all Spanish variants)
- `et`: Eesti
- `eu`: Euskara
- `fa`: فارسی (RTL; base locale for all Persian variants)
- `fa-AE`: فارسی (امارات) (RTL)
- `fa-AF`: دری (RTL)
- `fa-IR`: فارسی (ایران) (RTL)
- `fi`: Suomi
- `fil`: Filipino
- `fr`: Français (base locale for all French variants)
- `gl`: Galego
- `gu`: ગુજરાતી
- `he`: עברית (RTL)
- `hi`: हिन्दी
- `hr`: Hrvatski
- `hu`: Magyar
- `hy`: Հայերեն
- `id`: Bahasa Indonesia
- `is`: Íslenska
- `it`: Italiano
- `ja`: 日本語
- `ka`: ქართული
- `kk`: Қазақша
- `km`: ភាសាខ្មែរ
- `kn`: ಕನ್ನಡ
- `ko`: 한국어
- `ky`: Кыргызча
- `lo`: ລາວ
- `lt`: Lietuvių
- `lv`: Latviešu
- `mk`: Македонски
- `ml`: മലയാളം
- `mn`: Монгол
- `mr`: मराठी
- `ms`: Bahasa Melayu
- `my`: မြန်မာ
- `nb`: Norsk bokmål
- `ne`: नेपाली
- `nl`: Nederlands
- `nn`: Norsk nynorsk
- `pa`: ਪੰਜਾਬੀ
- `pl`: Polski
- `pt`: Português (base locale for all Portuguese variants)
- `rm`: Rumantsch
- `ro`: Română
- `ru`: Русский
- `si`: සිංහල
- `sk`: Slovenčina
- `sl`: Slovenščina
- `sq`: Shqip
- `sr`: Српски
- `sv`: Svenska
- `sw`: Kiswahili
- `ta`: தமிழ்
- `te`: తెలుగు
- `th`: ไทย
- `tr`: Türkçe
- `uk`: Українська
- `ur`: اردو (RTL)
- `vi`: Tiếng Việt
- `zh-CN`: 中文 (简体)
- `zh-HK`: 中文 (香港)
- `zh-TW`: 中文 (繁體)
- `zu`: isiZulu

## Base Language Strategy

Several supported locales are regional variants of the same language. To reduce duplication and maintenance overhead, use a **base translation file** for the shared language and only create locale-specific overrides where meaningful differences exist.

### How It Works

1. Create a complete translation file for the base locale (e.g., `en.json`).
2. For regional variants (e.g., `en-IN.json`), include **only the strings that differ** from the base.
3. The app should resolve strings by checking the specific locale first, then falling back to the base language.

For example, if the user's locale is `en-IN` and a key exists in `en-IN.json`, use it. If not, fall back to `en.json`.

### Locale Families

| Base Locale | Variants | Differences | Separate Files Needed? |
|-------------|----------|-------------|------------------------|
| `en` | `en-GB`, `en-AU`, `en-CA`, `en-IN`, `en-SG`, `en-ZA` | Mostly formatting (currency, date, numbers). en-IN uses lakhs/crores number system. UI strings are nearly identical. | No: use `en` as the base. Override only formatting via `Intl` API. Individual string overrides are rarely needed. |
| `fr-FR` | `fr-CA` | Minor vocabulary differences (e.g., "courriel" vs "e-mail"). Grammar and UI strings are mostly shared. | Minimal: base on `fr-FR`, override a small number of strings in `fr-CA`. |
| `es-ES` | `es-419`, `es-US` | Vocabulary differences (e.g., "ordenador" vs "computadora", "móvil" vs "celular"). Grammar differences with voseo in some Latin American regions. | Moderate: base on `es-ES`, override vocabulary-heavy strings in `es-419`. `es-US` can likely inherit from `es-419`. |
| `pt-PT` | `pt-BR` | More noticeable spelling and grammar differences (e.g., "facto" vs "fato", gerund usage). Still shares the majority of strings. | Moderate: base on one variant, override differing strings in the other. |
| `ms` | `ms-MY` | Effectively identical for formal/written contexts. | No: consolidate into `ms`. |
| `fa` | `fa-IR`, `fa-AE`, `fa-AF` | `fa` and `fa-IR` are identical. `fa-AF` (Dari) has vocabulary differences. `fa-AE` is a small diaspora variant. | Minimal: base on `fa`, override for `fa-AF` where Dari vocabulary differs. |

### Important: Chinese Is NOT a Candidate for This Approach

The Chinese variants `zh-CN` (Simplified), `zh-TW` (Traditional), and `zh-HK` (Traditional, Hong Kong) use **fundamentally different writing systems**, not just regional vocabulary. Simplified and Traditional Chinese have different character sets, and zh-HK includes Cantonese-influenced vocabulary distinct from zh-TW.

Each Chinese locale requires a **fully independent translation file**. Do not attempt to derive one from another through overrides.

### Other Notes on Script Differences

- **Serbian (`sr`)** can be written in both Cyrillic and Latin script. The current locale defaults to Cyrillic. If Latin script support is needed (common in digital contexts), consider adding `sr-Latn` as a separate locale.
- **Punjabi (`pa`)** uses Gurmukhi script (India) or Shahmukhi script (Pakistan, essentially Arabic script). The current locale assumes Gurmukhi. If Pakistani Punjabi users are a target audience, `pa-Arab` may be needed as a separate locale.