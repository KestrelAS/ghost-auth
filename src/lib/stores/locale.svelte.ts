import { locale } from 'svelte-i18n';
import { get } from 'svelte/store';
import { getSystemLocale, hasStoredLocale, clearStoredLocale } from '$lib/i18n';

const STORAGE_KEY = 'ghost-auth-locale';

let current: string = $state(get(locale) ?? 'en');
let isSystemDefault: boolean = $state(!hasStoredLocale());

locale.subscribe((val) => {
  current = val ?? 'en';
});

export function getLocale(): string {
  return current;
}

export function getIsSystemDefault(): boolean {
  return isSystemDefault;
}

export function setLocale(code: string) {
  locale.set(code);
  localStorage.setItem(STORAGE_KEY, code);
  isSystemDefault = false;
}

export function setSystemDefault() {
  clearStoredLocale();
  locale.set(getSystemLocale());
  isSystemDefault = true;
}
