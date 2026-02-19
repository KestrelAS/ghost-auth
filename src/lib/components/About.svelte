<script lang="ts">
  import { _ } from 'svelte-i18n';
  import ghostLogo from "$lib/assets/ghost.svg";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getVersion } from "@tauri-apps/api/app";

  let { onclose }: { onclose: () => void } = $props();

  let mounted = $state(false);
  let version = $state("");

  const REPO_URL = "https://github.com/KestrelAS/ghost-auth";
  const ISSUES_URL = "https://github.com/KestrelAS/ghost-auth/issues";

  $effect(() => {
    getVersion().then((v) => { version = v; }).catch(() => {});
  });

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
  }

  async function openLink(url: string) {
    try {
      await openUrl(url);
    } catch {
      // Silent fail
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-50 flex items-end sm:items-center justify-center modal-backdrop {mounted ? 'open' : ''}"
  onkeydown={(e) => e.key === "Escape" && close()}
  onclick={close}
  role="presentation"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="bg-bg border-t border-dotted border-border sm:border w-full max-w-md p-5 max-h-[85vh] overflow-y-auto modal-panel {mounted ? 'open' : ''}"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="about-title"
    tabindex="-1"
    use:trapFocus
  >
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <span id="about-title" class="text-base tracking-wide text-muted">{$_('about.title')}</span>
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

    <!-- Logo + Name -->
    <div class="flex flex-col items-center mb-6">
      <img src={ghostLogo} alt="" class="w-14 h-14 icon-adapt opacity-40 mb-4" />
      <h2 class="text-lg tracking-wider text-fg/80">{$_('about.heading')}</h2>
      {#if version}<span class="text-xs text-dim mt-1 tracking-wider">{$_('about.version', { values: { version } })}</span>{/if}
    </div>

    <!-- Description -->
    <div class="border border-dotted border-border px-4 py-3 mb-4">
      <p class="text-sm text-muted leading-relaxed">
        {$_('about.description')}
      </p>
    </div>

    <!-- Links -->
    <div class="flex flex-col gap-2 mb-4">
      <button
        type="button"
        class="w-full text-left border border-dotted border-border px-4 py-2.5 hover:border-fg/30 transition-colors group"
        onclick={() => openLink(REPO_URL)}
      >
        <span class="text-sm text-dim group-hover:text-fg transition-colors">{$_('about.sourceCode')}</span>
        <span class="text-xs text-dim block mt-0.5 truncate">{$_('about.sourceCodeUrl')}</span>
      </button>
      <button
        type="button"
        class="w-full text-left border border-dotted border-border px-4 py-2.5 hover:border-fg/30 transition-colors group"
        onclick={() => openLink(ISSUES_URL)}
      >
        <span class="text-sm text-dim group-hover:text-fg transition-colors">{$_('about.reportIssue')}</span>
        <span class="text-xs text-dim block mt-0.5">{$_('about.reportIssueDesc')}</span>
      </button>
    </div>

    <!-- Footer -->
    <div class="text-center">
      <span class="text-xs text-dim tracking-wider">{$_('about.license')}</span>
      <span class="text-xs text-dim/60 mx-1.5">/</span>
      <span class="text-xs text-dim tracking-wider">{$_('about.madeBy')}</span>
    </div>
  </div>
</div>
