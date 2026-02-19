<script lang="ts">
  import { _ } from 'svelte-i18n';
  import Fuse from 'fuse.js';
  import { trapFocus } from "$lib/utils/focusTrap";
  import { getLocale, getIsSystemDefault, setLocale, setSystemDefault } from "$lib/stores/locale.svelte";
  import { LANGUAGES } from "$lib/i18n";

  let { onclose }: { onclose: () => void } = $props();

  let search = $state("");
  let mounted = $state(false);
  let currentLocale = $derived(getLocale());
  let isSystemLang = $derived(getIsSystemDefault());

  const fuse = new Fuse(LANGUAGES, {
    keys: [
      { name: 'english', weight: 2 },
      { name: 'name', weight: 1.5 },
      { name: 'code', weight: 1 },
    ],
    threshold: 0.35,
    ignoreLocation: true,
    minMatchCharLength: 1,
  });

  let filtered = $derived(
    search.trim()
      ? fuse.search(search.trim()).map(r => r.item)
      : LANGUAGES,
  );

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
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
    class="bg-bg border-t border-dotted border-border w-full p-5 max-h-[85vh] flex flex-col modal-panel {mounted ? 'open' : ''}"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="language-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-4">
      <span id="language-title" class="text-base tracking-wide text-muted">> {$_('languageSelect.title')}</span>
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

    <!-- Search -->
    <div class="mb-3">
      <input
        type="text"
        bind:value={search}
        placeholder={$_('app.searchPlaceholder')}
        class="w-full bg-transparent text-fg text-xs border border-dotted border-border px-3 py-2 outline-none focus:border-fg/40 transition-colors placeholder:text-dim"
      />
    </div>

    <!-- Language list -->
    <div class="flex-1 overflow-y-auto flex flex-col gap-1">
      <!-- System default -->
      {#if !search.trim()}
        <button
          type="button"
          class="w-full text-left border px-4 py-2.5 transition-colors flex items-center gap-2.5 {isSystemLang ? 'border-fg/60 text-fg' : 'border-dotted border-border text-dim hover:text-fg hover:border-fg/30'}"
          onclick={() => { setSystemDefault(); close(); }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50 shrink-0">
            <circle cx="12" cy="12" r="10" />
            <line x1="2" y1="12" x2="22" y2="12" />
            <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
          </svg>
          <span class="text-sm">{$_('settings.systemDefault')}</span>
        </button>
      {/if}

      {#each filtered as lang}
        <button
          type="button"
          class="w-full text-left border px-4 py-2.5 transition-colors {!isSystemLang && currentLocale === lang.code ? 'border-fg/60 text-fg' : 'border-dotted border-border text-dim hover:text-fg hover:border-fg/30'}"
          onclick={() => { setLocale(lang.code); close(); }}
        >
          <span class="text-sm">
            {lang.name}{#if lang.name !== lang.english}<span class="opacity-40 ml-2">{lang.english}</span>{/if}
          </span>
        </button>
      {/each}

      {#if filtered.length === 0}
        <div class="py-8 text-center">
          <p class="text-xs text-muted">{$_('languageSelect.noMatches', { values: { search } })}</p>
        </div>
      {/if}
    </div>
  </div>
</div>
