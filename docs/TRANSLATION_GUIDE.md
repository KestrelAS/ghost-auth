# Translation Guide

Ghost Auth supports 79 languages. Many of these translations were initially generated with AI assistance and need review by native speakers. This guide explains how to contribute translation improvements.

## What We Need

Most locale files already exist; we're not starting from scratch. What we need is **native speakers reviewing and correcting existing translations**. Common issues include:

- Awkward phrasing that reads like machine translation
- Wrong register (too casual or too formal for the context)
- Technical terms that have better local equivalents
- Placeholder variables that ended up in the wrong position for the language's grammar
- Missing or incorrect plural forms

## How Translations Work

Each language has a JSON file at:

```
src/lib/i18n/locales/{code}.json       # Main app
```

The English source file (`en.json`) is the reference: every key that exists in `en.json` must also exist in your locale file.

### File Format

Translations are flat-ish JSON with nested sections:

```json
{
  "common": {
    "close": "Schließen",
    "save": "Speichern"
  },
  "lockScreen": {
    "enterPin": "PIN eingeben",
    "wrongPin": "Falscher PIN"
  }
}
```

### Placeholders

Some strings contain `{variables}` that get replaced at runtime. Keep them exactly as-is: don't translate, rename, or remove them. You can move them to wherever they fit naturally in your language:

```
English:  "{count} codes remaining"
Japanese: "残りのコード: {count}"
German:   "{count} Codes übrig"
```

### Plurals

Some strings use ICU MessageFormat for plurals:

```
{count, plural, one {# code remaining} other {# codes remaining}}
```

Different languages need different plural forms. English has 2 (one, other). Arabic has 6. Japanese has 1. Provide all the forms your language requires.

## Step-by-Step: Reviewing a Translation

1. **Fork and clone** the repository
2. **Open your locale file**: `src/lib/i18n/locales/{your-code}.json`
3. **Compare against `en.json`** side by side: every key must be present
4. **Read each string in context**. Ask yourself:
   - Does this sound natural to a native speaker?
   - Is the formality level appropriate for a security app? (Use formal register: "Sie" not "du", "vous" not "tu", etc.)
   - Are technical terms handled correctly? Keep "Ghost Auth", "TOTP", "QR", "PIN", "SHA-256", "Base32" untranslated.
   - Would a different word or phrasing be clearer?
5. **Test your changes** by running the app in your locale:
   ```bash
   npm install
   npm run tauri dev
   ```
   Then switch to your language in Settings. Check that strings fit in buttons, labels, and modals without overflowing.
6. **Copy your changes** to the extension locale file at `extension/src/lib/i18n/locales/{your-code}.json`: both must match
7. **Submit a PR** with a clear title like: `i18n: improve German translations`

## Step-by-Step: Adding a New Language

If your language isn't supported yet:

1. Copy `en.json` to `{your-code}.json` in both `src/lib/i18n/locales/` and `extension/src/lib/i18n/locales/`
2. Translate all strings
3. Register the locale in both `src/lib/i18n/index.ts` and `extension/src/lib/i18n/index.ts`:
   ```ts
   register('xx', () => import('./locales/xx.json'));
   ```
4. Add the language to the `LANGUAGES` array in both files:
   ```ts
   { code: 'xx', name: 'Your Language Name' }
   ```
   The `name` must be written in the language's own script (e.g., "Deutsch", not "German")
5. If your language is RTL, add the code to `RTL_LOCALES` in `src/lib/stores/locale.svelte.ts`

See [ADDING_LANGUAGE_SUPPORT.md](ADDING_LANGUAGE_SUPPORT.md) for detailed technical guidance on locale codes, plural forms, RTL handling, and regional variants.

## Tone and Register

Ghost Auth is a security app. Users trust it with their authentication credentials. Translations should sound **professional and clear**: not robotic, not chatty.

- Use the **formal register** where the language distinguishes (Sie, vous, usted, です/ます, 합니다)
- Prefer **concise, direct phrasing**: button labels and error messages should be short
- When a technical term has no natural equivalent, keep the English term. Don't force a translation that would confuse users.

## Quality Checklist

Before submitting:

- [ ] All keys from `en.json` are present
- [ ] No `{placeholders}` are missing or renamed
- [ ] Plural forms are complete for your language
- [ ] Formal register is used where applicable
- [ ] Strings fit in the UI without overflow (test in the app)
- [ ] RTL rendering works correctly (if applicable)

## Questions?

Open an issue with the `i18n` label if you're unsure about a translation choice or need context for a specific string.
