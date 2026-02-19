<script lang="ts">
  import { toggleTheme, getTheme } from "$lib/stores/theme.svelte";
  import { storage } from "$lib/stores/accounts.svelte";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { _ } from 'svelte-i18n';
  import { getLocale, getIsSystemDefault } from '$lib/stores/locale.svelte';
  import { LANGUAGES } from '$lib/i18n';
  import iconLock from "$lib/assets/icons/lock.svg";
  import iconUnlock from "$lib/assets/icons/unlock.svg";
  import iconExport from "$lib/assets/icons/export.svg";
  import iconImport from "$lib/assets/icons/import.svg";
  import iconAbout from "$lib/assets/icons/about.svg";
  import iconPhone from "$lib/assets/icons/iphone.svg";
  import iconArrow from "$lib/assets/icons/right-arrow.svg";

  let { onclose, onlock, onexport, onimport, onpintoggle, onsync, onhelp, onlanguage, pinEnabled, autoLockMinutes, onautolockchange, passwordlessEnabled, onpasswordlesstoggle }: {
    onclose: () => void;
    onlock: () => void;
    onexport: () => void;
    onimport: () => void;
    onpintoggle: () => void;
    onsync: () => void;
    onhelp: () => void;
    onlanguage: () => void;
    pinEnabled: boolean;
    autoLockMinutes: number;
    onautolockchange: (minutes: number) => void;
    passwordlessEnabled: boolean;
    onpasswordlesstoggle: (enabled: boolean) => void;
  } = $props();

  let currentLanguageName = $derived(
    getIsSystemDefault()
      ? $_('settings.systemDefault')
      : (LANGUAGES.find(l => l.code === getLocale())?.name ?? getLocale())
  );

  let mounted = $state(false);
  let showPasswordlessWarning = $state(false);

  let autoLockOptions = $derived([
    { label: $_('ext.settings.autoLock1'), value: 1 },
    { label: $_('ext.settings.autoLock5'), value: 5 },
    { label: $_('ext.settings.autoLock15'), value: 15 },
    { label: $_('ext.settings.autoLock30'), value: 30 },
    { label: $_('ext.settings.autoLockNever'), value: 0 },
  ]);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
  }

  function handleLock() {
    storage.lock();
    close();
    setTimeout(onlock, 300);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-50 flex items-end justify-center modal-backdrop {mounted ? 'open' : ''}"
  onkeydown={(e) => e.key === "Escape" && close()}
  onclick={close}
  role="presentation"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="bg-bg border-t border-dotted border-border w-full p-5 max-h-[85vh] overflow-y-auto modal-panel {mounted ? 'open' : ''}"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="settings-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-5">
      <span id="settings-title" class="text-base tracking-wide text-muted">> {$_('settings.title')}</span>
      <button
        type="button"
        class="text-dim hover:text-fg transition-colors p-1"
        onclick={close}
        aria-label={$_('common.close')}
      >
        <svg width="18" height="18" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <line x1="2" y1="2" x2="12" y2="12" /><line x1="12" y1="2" x2="2" y2="12" />
        </svg>
      </button>
    </div>

    <div class="flex flex-col gap-1">
      <!-- Theme toggle -->
      <button
        type="button"
        class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
        onclick={toggleTheme}
      >
        <div class="text-sm text-fg flex items-center gap-3">
          {#if getTheme() === 'dark'}
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
          {getTheme() === 'dark' ? $_('settings.darkMode') : $_('settings.lightMode')}
        </div>
        <div class="text-xs text-dim mt-1 ml-7">{$_('ext.settings.themeDesc')}</div>
      </button>

      <!-- Language -->
      <button
        type="button"
        class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
        onclick={() => { close(); setTimeout(onlanguage, 300); }}
      >
        <div class="text-sm text-fg flex items-center justify-between">
          <span class="flex items-center gap-3">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50">
              <circle cx="12" cy="12" r="10" />
              <line x1="2" y1="12" x2="22" y2="12" />
              <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
            </svg>
            {currentLanguageName}
          </span>
          <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-40">
            <path d="M8 4l6 6-6 6" />
          </svg>
        </div>
      </button>

      <!-- Security section -->
      <div class="text-[10px] text-dim tracking-widest mt-4 mb-1 px-1">{$_('ext.settings.sectionSecurity')}</div>

      <!-- PIN lock toggle -->
      <button
        type="button"
        class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
        onclick={() => { close(); setTimeout(onpintoggle, 300); }}
      >
        <div class="text-sm text-fg flex items-center justify-between">
          <span class="flex items-center gap-3">
            <img src={pinEnabled ? iconLock : iconUnlock} alt="" class="w-4 h-4 icon-adapt opacity-50" />
            {$_('settings.pinLock')}
          </span>
          <span class="text-xs text-dim">{pinEnabled ? $_('ext.settings.pinEnabled') : $_('ext.settings.pinDisabled')}</span>
        </div>
        <div class="text-xs text-dim mt-1 ml-7">
          {pinEnabled ? $_('ext.settings.pinRemoveHint') : $_('ext.settings.pinAddHint')}
        </div>
      </button>

      <!-- Auto-lock timeout (only visible when PIN is enabled) -->
      {#if pinEnabled}
        <div class="border border-dotted border-border px-4 py-3">
          <div class="text-sm text-fg mb-2">{$_('ext.settings.autoLockLabel')}</div>
          <div class="flex flex-wrap gap-1.5">
            {#each autoLockOptions as opt}
              <button
                type="button"
                class="text-xs px-2.5 py-1 border border-dotted transition-colors {autoLockMinutes === opt.value ? 'border-fg/60 text-fg' : 'border-border text-dim hover:border-fg/30 hover:text-fg'}"
                onclick={() => onautolockchange(opt.value)}
              >
                {opt.label}
              </button>
            {/each}
          </div>
          <div class="text-xs text-dim mt-2">
            {autoLockMinutes === 0 ? $_('ext.settings.autoLockNeverDesc') : $_('ext.settings.autoLockDesc', { values: { minutes: autoLockMinutes } })}
          </div>
        </div>
      {/if}

      <!-- Passwordless toggle -->
      <button
        type="button"
        class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
        onclick={() => {
          if (passwordlessEnabled) {
            onpasswordlesstoggle(false);
          } else {
            showPasswordlessWarning = !showPasswordlessWarning;
          }
        }}
      >
        <div class="text-sm text-fg flex items-center justify-between">
          <span class="flex items-center gap-3">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
            </svg>
            {$_('ext.settings.passwordlessLabel')}
          </span>
          <span class="text-xs text-dim">{passwordlessEnabled ? $_('ext.settings.pinEnabled') : $_('ext.settings.pinDisabled')}</span>
        </div>
        <div class="text-xs text-dim mt-1 ml-7">
          {passwordlessEnabled ? $_('ext.settings.passwordlessEnabledHint') : $_('ext.settings.passwordlessDisabledHint')}
        </div>
      </button>

      {#if showPasswordlessWarning}
        <div class="border border-dotted border-error/30 px-4 py-3">
          <div class="text-xs text-error/80 mb-2">{$_('ext.settings.passwordlessWarning')}</div>
          <div class="text-xs text-dim leading-relaxed">
            {$_('ext.settings.passwordlessWarningDesc')}
          </div>
          <button
            type="button"
            class="mt-3 w-full border border-dotted border-error/40 text-error text-xs py-2 hover:border-error/60 transition-colors"
            onclick={() => {
              showPasswordlessWarning = false;
              onpasswordlesstoggle(true);
            }}
          >
            {$_('ext.settings.passwordlessConfirm')}
          </button>
        </div>
      {/if}

      <!-- Lock vault -->
      {#if !passwordlessEnabled}
        <button
          type="button"
          class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
          onclick={handleLock}
        >
          <div class="text-sm text-fg flex items-center gap-3">
            <img src={iconLock} alt="" class="w-4 h-4 icon-adapt opacity-50" />
            {$_('ext.settings.lockVault')}
          </div>
          <div class="text-xs text-dim mt-1 ml-7">{pinEnabled ? $_('ext.settings.lockPinDesc') : $_('ext.settings.lockPasswordDesc')}</div>
        </button>
      {/if}

      <!-- Sync section -->
      <div class="text-[10px] text-dim tracking-widest mt-4 mb-1 px-1">{$_('ext.settings.sectionSync')}</div>

      <button
        type="button"
        class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
        onclick={() => { close(); setTimeout(onsync, 300); }}
      >
        <div class="text-sm text-fg flex items-center gap-3">
          <span class="flex items-center gap-1">
            <img src={iconArrow} alt="" class="w-2.5 h-2.5 icon-adapt opacity-35" style="transform: scaleX(-1)" />
            <img src={iconPhone} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
          </span>
          {$_('ext.settings.syncFromDevice')}
        </div>
        <div class="text-xs text-dim mt-1 ml-7">{$_('ext.settings.syncFromDeviceDesc')}</div>
      </button>

      <!-- Backup section -->
      <div class="text-[10px] text-dim tracking-widest mt-4 mb-1 px-1">{$_('ext.settings.sectionBackup')}</div>

      <button
        type="button"
        class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
        onclick={() => { close(); setTimeout(onexport, 300); }}
      >
        <div class="text-sm text-fg flex items-center gap-3">
          <img src={iconExport} alt="" class="w-4 h-4 icon-adapt opacity-50" />
          {$_('ext.settings.exportLabel')}
        </div>
        <div class="text-xs text-dim mt-1 ml-7">{$_('ext.settings.exportDesc')}</div>
      </button>

      <button
        type="button"
        class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
        onclick={() => { close(); setTimeout(onimport, 300); }}
      >
        <div class="text-sm text-fg flex items-center gap-3">
          <img src={iconImport} alt="" class="w-4 h-4 icon-adapt opacity-50" />
          {$_('ext.settings.importLabel')}
        </div>
        <div class="text-xs text-dim mt-1 ml-7">{$_('ext.settings.importDesc')}</div>
      </button>
    </div>

    <div class="mt-5 flex flex-col gap-1">
      <button
        type="button"
        class="w-full flex items-center justify-center gap-2 text-dim text-xs tracking-wide py-3 hover:text-fg transition-colors"
        onclick={() => { close(); setTimeout(onhelp, 300); }}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-40">
          <circle cx="12" cy="12" r="10" />
          <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
        </svg>
        {$_('ext.settings.helpLabel')}
      </button>
      <div class="flex items-center justify-center gap-2 py-2">
        <img src={iconAbout} alt="" class="w-3.5 h-3.5 icon-adapt opacity-40" />
        <span class="text-[10px] text-dim tracking-widest">{$_('ext.settings.version', { values: { version: '1.0.0' } })}</span>
      </div>
    </div>
  </div>
</div>
