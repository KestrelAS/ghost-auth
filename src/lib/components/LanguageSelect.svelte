<script lang="ts">
  import { _ } from 'svelte-i18n';
  import Fuse from 'fuse.js';
  import { trapFocus } from "$lib/utils/focusTrap";
  import { swipeBack } from "$lib/utils/swipeBack";
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
    aria-labelledby="language-title"
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
      <span id="language-title" class="text-lg tracking-wide text-muted">{$_('languageSelect.title')}</span>
    </div>

    <!-- Content -->
    <div class="max-w-md mx-auto w-full px-5 py-4 flex flex-col flex-1 overflow-y-auto" style="padding-bottom: 5rem;">
      <!-- Language list -->
      <div class="flex flex-col gap-1.5">
        <!-- System default (always visible) -->
        {#if !search.trim()}
          <button
            type="button"
            class="w-full text-left border px-4 py-3 transition-colors flex items-center justify-between {isSystemLang ? 'border-fg/80 text-fg' : 'border-dotted border-border text-dim hover:text-fg hover:border-fg/30'}"
            onclick={setSystemDefault}
          >
            <span class="flex items-center gap-3 text-sm">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-50">
                <circle cx="12" cy="12" r="10" />
                <line x1="2" y1="12" x2="22" y2="12" />
                <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
              </svg>
              {$_('settings.systemDefault')}
            </span>
          </button>
        {/if}

        {#each filtered as lang}
          <button
            type="button"
            class="w-full text-left border px-4 py-3 transition-colors flex items-center justify-between {!isSystemLang && currentLocale === lang.code ? 'border-fg/80 text-fg' : 'border-dotted border-border text-dim hover:text-fg hover:border-fg/30'}"
            onclick={() => setLocale(lang.code)}
          >
            <span class="text-sm pl-[28px]">
              {lang.name}{#if lang.name !== lang.english}
                <span class="opacity-40 ml-2.5">{lang.english}</span>
              {/if}
            </span>
          </button>
        {/each}

        {#if filtered.length === 0}
          <div class="flex flex-col items-center justify-center py-16 text-dim">
            <p class="text-xs text-muted">{$_('languageSelect.noMatches', { values: { search } })}</p>
          </div>
        {/if}
      </div>
    </div>

    <!-- Search bar (bottom, hovering) -->
    <div class="fixed left-6 right-6 z-20 h-16 flex items-center search-bottom">
      <input
        type="text"
        bind:value={search}
        placeholder={$_('app.searchPlaceholder')}
        class="w-full bg-bg/60 backdrop-blur-md shadow-lg text-fg border border-dotted border-border px-3 py-2 text-sm outline-none focus:border-fg/40 transition-colors placeholder:text-dim"
      />
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
