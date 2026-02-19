<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { trapFocus } from "$lib/utils/focusTrap";
  import { swipeBack } from "$lib/utils/swipeBack";
  import { getTheme, toggleTheme } from "$lib/stores/theme.svelte";
  import { getLocale, getIsSystemDefault } from "$lib/stores/locale.svelte";
  import { LANGUAGES } from "$lib/i18n";
  import iconLock from "$lib/assets/icons/lock.svg";
  import iconUnlock from "$lib/assets/icons/unlock.svg";
  import iconExport from "$lib/assets/icons/export.svg";
  import iconImport from "$lib/assets/icons/import.svg";
  import iconPhone from "$lib/assets/icons/iphone.svg";
  import iconArrow from "$lib/assets/icons/right-arrow.svg";
  import iconApp from "$lib/assets/icons/app.svg";
  import iconAbout from "$lib/assets/icons/about.svg";
  import iconQr from "$lib/assets/icons/qr.svg";

  let {
    onclose,
    pinEnabled,
    biometricEnabled,
    biometricHardwareAvailable,
    onbiometrictoggle,
    onpintoggle,
    onexport,
    onimport,
    onimportexternal,
    onexportqr,
    onsyncto,
    onsyncfrom,
    onabout,
    onhelp,
    onlanguage,
  }: {
    onclose: () => void;
    pinEnabled: boolean;
    biometricEnabled: boolean;
    biometricHardwareAvailable: boolean;
    onbiometrictoggle: () => void;
    onpintoggle: () => void;
    onexport: () => void;
    onimport: () => void;
    onimportexternal: () => void;
    onexportqr: () => void;
    onsyncto: () => void;
    onsyncfrom: () => void;
    onabout: () => void;
    onhelp: () => void;
    onlanguage: () => void;
  } = $props();

  let theme = $derived(getTheme());
  let currentLocale = $derived(getLocale());
  let isSystemLang = $derived(getIsSystemDefault());
  let currentLangName = $derived(
    isSystemLang
      ? $_('settings.systemDefault')
      : LANGUAGES.find((l) => l.code === currentLocale)?.name ?? currentLocale,
  );
  let mounted = $state(false);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 300);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-40 settings-backdrop {mounted ? 'open' : ''}"
  onclick={close}
  onkeydown={(e) => e.key === "Escape" && close()}
  role="presentation"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 bg-bg settings-slide {mounted ? 'open' : ''} flex flex-col pt-safe pb-safe"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="settings-title"
    tabindex="-1"
    use:trapFocus
    use:swipeBack={{ onclose: () => setTimeout(onclose, 0) }}
  >
    <!-- Header -->
    <div class="max-w-md mx-auto w-full px-5 py-4 flex items-center gap-3 border-dotted-b">
      <button
        type="button"
        class="text-dim hover:text-fg transition-colors p-1"
        onclick={close}
        aria-label={$_('common.back')}
      >
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 4l-6 6 6 6" />
        </svg>
      </button>
      <span id="settings-title" class="text-lg tracking-wide text-muted">{$_('settings.title')}</span>
    </div>

    <!-- Content -->
    <div class="max-w-md mx-auto w-full px-5 py-6 flex flex-col gap-6 flex-1 overflow-y-auto">
      <!-- Appearance -->
      <div>
        <p class="text-xs text-dim tracking-wide mb-3">{$_('settings.appearance')}</p>
        <div class="flex items-center justify-between border border-dotted border-border px-4 py-3">
          <span class="flex items-center gap-3 text-sm text-muted">
            {#if theme === 'dark'}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50">
                <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
              </svg>
            {:else}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50">
                <circle cx="12" cy="12" r="5" />
                <line x1="12" y1="1" x2="12" y2="3" /><line x1="12" y1="21" x2="12" y2="23" />
                <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" /><line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
                <line x1="1" y1="12" x2="3" y2="12" /><line x1="21" y1="12" x2="23" y2="12" />
                <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" /><line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
              </svg>
            {/if}
            {theme === 'dark' ? $_('settings.darkMode') : $_('settings.lightMode')}
          </span>
          <button
            type="button"
            class="w-11 h-6 rounded-full border transition-all duration-200 relative {theme === 'light' ? 'bg-accent/15 border-accent/40' : 'bg-transparent border-dim/50'}"
            onclick={toggleTheme}
            role="switch"
            aria-checked={theme === 'light'}
            aria-label={$_('settings.toggleLight')}
          >
            <div class="w-4 h-4 rounded-full absolute top-0.5 transition-all duration-200 {theme === 'light' ? 'left-[22px] bg-accent' : 'left-[3px] bg-dim'}"></div>
          </button>
        </div>
      </div>

      <!-- Language -->
      <div>
        <p class="text-xs text-dim tracking-wide mb-3">{$_('settings.language')}</p>
        <button
          type="button"
          class="w-full text-left border border-dotted border-border px-4 py-3 text-sm text-dim hover:text-fg hover:border-fg/30 transition-colors flex items-center justify-between"
          onclick={onlanguage}
        >
          <span class="flex items-center gap-3">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50">
              <circle cx="12" cy="12" r="10" />
              <line x1="2" y1="12" x2="22" y2="12" />
              <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
            </svg>
            {currentLangName}
          </span>
          <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-40">
            <path d="M8 4l6 6-6 6" />
          </svg>
        </button>
      </div>

      <!-- Security -->
      <div>
        <p class="text-xs text-dim tracking-wide mb-3">{$_('settings.security')}</p>
        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between border border-dotted border-border px-4 py-3">
            <span class="flex items-center gap-3 text-sm text-muted">
              <img src={pinEnabled ? iconLock : iconUnlock} alt="" class="w-4 h-4 icon-adapt opacity-50" />
              {$_('settings.pinLock')}
            </span>
            <button
              type="button"
              class="w-11 h-6 rounded-full border transition-all duration-200 relative {pinEnabled ? 'bg-accent/15 border-accent/40' : 'bg-transparent border-dim/50'}"
              onclick={onpintoggle}
              role="switch"
              aria-checked={pinEnabled}
              aria-label={$_('settings.togglePin')}
            >
              <div class="w-4 h-4 rounded-full absolute top-0.5 transition-all duration-200 {pinEnabled ? 'left-[22px] bg-accent' : 'left-[3px] bg-dim'}"></div>
            </button>
          </div>
          {#if pinEnabled && biometricHardwareAvailable}
            <div class="flex items-center justify-between border border-dotted border-border px-4 py-3">
              <span class="flex items-center gap-3 text-sm text-muted">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50">
                  <path d="M7.5 11c0-2.5 2-4.5 4.5-4.5s4.5 2 4.5 4.5" />
                  <path d="M12 12v4" />
                  <path d="M5 10c0-3.9 3.1-7 7-7s7 3.1 7 7" />
                  <path d="M3 9c0-5 4-9 9-9s9 4 9 9" />
                  <path d="M10 12c0-1.1.9-2 2-2s2 .9 2 2v3c0 1.1-.9 2-2 2" />
                  <path d="M7.5 15c0 2.5 2 4.5 4.5 4.5 1.4 0 2.6-.6 3.4-1.6" />
                  <path d="M5 14c0 3.9 3.1 7 7 7 2.8 0 5.2-1.6 6.3-4" />
                </svg>
                {$_('settings.biometricUnlock')}
              </span>
              <button
                type="button"
                class="w-11 h-6 rounded-full border transition-all duration-200 relative {biometricEnabled ? 'bg-accent/15 border-accent/40' : 'bg-transparent border-dim/50'}"
                onclick={onbiometrictoggle}
                role="switch"
                aria-checked={biometricEnabled}
                aria-label={$_('settings.toggleBiometric')}
              >
                <div class="w-4 h-4 rounded-full absolute top-0.5 transition-all duration-200 {biometricEnabled ? 'left-[22px] bg-accent' : 'left-[3px] bg-dim'}"></div>
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Sync -->
      <div>
        <p class="text-xs text-dim tracking-wide mb-3">{$_('settings.sync')}</p>
        <div class="flex flex-col gap-1.5">
          <button
            type="button"
            class="w-full text-left border border-dotted border-border px-4 py-3 text-sm text-dim hover:text-fg hover:border-fg/30 transition-colors flex items-center gap-3"
            onclick={onsyncto}
          >
            <span class="flex items-center gap-1">
              <img src={iconArrow} alt="" class="w-2.5 h-2.5 icon-adapt opacity-35" />
              <img src={iconPhone} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
            </span>
            {$_('settings.syncTo')}
          </button>
          <button
            type="button"
            class="w-full text-left border border-dotted border-border px-4 py-3 text-sm text-dim hover:text-fg hover:border-fg/30 transition-colors flex items-center gap-3"
            onclick={onsyncfrom}
          >
            <span class="flex items-center gap-1">
              <img src={iconArrow} alt="" class="w-2.5 h-2.5 icon-adapt opacity-35" style="transform: scaleX(-1)" />
              <img src={iconPhone} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
            </span>
            {$_('settings.syncFrom')}
          </button>
        </div>
      </div>

      <!-- Import -->
      <div>
        <p class="text-xs text-dim tracking-wide mb-3">{$_('settings.import')}</p>
        <button
          type="button"
          class="w-full text-left border border-dotted border-border px-4 py-3 text-sm text-dim hover:text-fg hover:border-fg/30 transition-colors flex items-center gap-3"
          onclick={onimportexternal}
        >
          <span class="flex items-center gap-1">
            <img src={iconApp} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
            <img src={iconArrow} alt="" class="w-2.5 h-2.5 icon-adapt opacity-35" />
          </span>
          {$_('settings.importFromApp')}
        </button>
      </div>

      <!-- Export -->
      <div>
        <p class="text-xs text-dim tracking-wide mb-3">{$_('settings.export')}</p>
        <button
          type="button"
          class="w-full text-left border border-dotted border-border px-4 py-3 text-sm text-dim hover:text-fg hover:border-fg/30 transition-colors flex items-center gap-3"
          onclick={onexportqr}
        >
          <img src={iconQr} alt="" class="w-4 h-4 icon-adapt opacity-50" />
          {$_('settings.exportQr')}
        </button>
      </div>

      <!-- Backup -->
      <div>
        <p class="text-xs text-dim tracking-wide mb-3">{$_('settings.backup')}</p>
        <div class="flex flex-col gap-1.5">
          <button
            type="button"
            class="w-full text-left border border-dotted border-border px-4 py-3 text-sm text-dim hover:text-fg hover:border-fg/30 transition-colors flex items-center gap-3"
            onclick={onimport}
          >
            <img src={iconImport} alt="" class="w-4 h-4 icon-adapt opacity-50" />
            {$_('settings.importBackup')}
          </button>
          <button
            type="button"
            class="w-full text-left border border-dotted border-border px-4 py-3 text-sm text-dim hover:text-fg hover:border-fg/30 transition-colors flex items-center gap-3"
            onclick={onexport}
          >
            <img src={iconExport} alt="" class="w-4 h-4 icon-adapt opacity-50" />
            {$_('settings.exportBackup')}
          </button>
        </div>
      </div>

      <!-- Help & About — pushed to bottom -->
      <div class="mt-auto pt-4 flex flex-col gap-1">
        <button
          type="button"
          class="w-full flex items-center justify-center gap-2 text-dim text-xs tracking-wide py-3 hover:text-fg transition-colors"
          onclick={onhelp}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-40">
            <circle cx="12" cy="12" r="10" />
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
          {$_('settings.help')}
        </button>
        <button
          type="button"
          class="w-full flex items-center justify-center gap-2 text-dim text-xs tracking-wide py-3 hover:text-fg transition-colors"
          onclick={onabout}
        >
          <img src={iconAbout} alt="" class="w-3.5 h-3.5 icon-adapt opacity-40" />
          {$_('settings.about')}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .settings-backdrop {
    background: var(--color-backdrop-light);
    opacity: 0;
    transition: opacity 0.3s ease;
  }
  .settings-backdrop.open {
    opacity: 1;
  }
  .settings-slide {
    transform: translateX(100%);
    transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }
  .settings-slide.open {
    transform: translateX(0);
  }
</style>
