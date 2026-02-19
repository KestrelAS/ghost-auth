<script lang="ts">
  import { _ } from 'svelte-i18n';
  let { onenable, onskip }: { onenable: () => void; onskip: () => void } = $props();

  let mounted = $state(false);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });
</script>

<div
  class="fixed inset-0 z-50 bg-bg flex flex-col items-center justify-center select-none pt-safe pb-safe transition-opacity duration-300 {mounted ? 'opacity-100' : 'opacity-0'}"
>
  <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-fg/40 mb-8">
    <path d="M7.5 11c0-2.5 2-4.5 4.5-4.5s4.5 2 4.5 4.5" />
    <path d="M12 12v4" />
    <path d="M5 10c0-3.9 3.1-7 7-7s7 3.1 7 7" />
    <path d="M3 9c0-5 4-9 9-9s9 4 9 9" />
    <path d="M10 12c0-1.1.9-2 2-2s2 .9 2 2v3c0 1.1-.9 2-2 2" />
    <path d="M7.5 15c0 2.5 2 4.5 4.5 4.5 1.4 0 2.6-.6 3.4-1.6" />
    <path d="M5 14c0 3.9 3.1 7 7 7 2.8 0 5.2-1.6 6.3-4" />
  </svg>

  <h1 class="text-lg tracking-wider text-fg/80 mb-3">{$_('biometricPrompt.title')}</h1>

  <p class="text-sm text-dim text-center max-w-xs px-4 mb-10 leading-relaxed">
    {$_('biometricPrompt.description')}
  </p>

  <div class="flex flex-col gap-3 w-full max-w-[200px]">
    <button
      type="button"
      class="w-full border border-fg/80 text-fg text-sm py-3 hover:bg-fg hover:text-bg transition-colors"
      onclick={onenable}
    >
      {$_('biometricPrompt.enable')}
    </button>
    <button
      type="button"
      class="w-full border border-dotted border-border text-dim text-sm py-3 hover:text-fg hover:border-fg/30 transition-colors"
      onclick={onskip}
    >
      {$_('biometricPrompt.notNow')}
    </button>
  </div>
</div>
