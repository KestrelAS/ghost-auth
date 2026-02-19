<script lang="ts">
  import { trapFocus } from "$lib/utils/focusTrap";
  import { _ } from 'svelte-i18n';

  let { onclose }: { onclose: () => void } = $props();

  let mounted = $state(false);

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
    class="bg-bg border-t border-dotted border-border w-full p-5 max-h-[85vh] overflow-y-auto modal-panel {mounted ? 'open' : ''}"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="help-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-5">
      <span id="help-title" class="text-base tracking-wide text-muted">{$_('ext.help.title')}</span>
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

    <div class="flex flex-col gap-5">
      <!-- Syncing -->
      <div>
        <div class="flex items-center gap-2 text-[10px] text-dim tracking-widest mb-2 px-1">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-accent opacity-70">
            <path d="M17 1l4 4-4 4" /><path d="M3 11V9a4 4 0 0 1 4-4h14" />
            <path d="M7 23l-4-4 4-4" /><path d="M21 13v2a4 4 0 0 1-4 4H3" />
          </svg>
          {$_('ext.help.syncingTitle')}
        </div>
        <div class="border border-dotted border-border px-4 py-3 flex flex-col gap-2">
          <p class="text-sm text-muted leading-relaxed">
            {$_('ext.help.syncingDesc')}
          </p>
          <p class="text-sm text-muted leading-relaxed">
            {$_('ext.help.syncingStep')}
          </p>
          <p class="text-xs text-dim leading-relaxed mt-1">
            {$_('ext.help.syncingNote')}
          </p>
        </div>
      </div>

      <!-- Backups -->
      <div>
        <div class="flex items-center gap-2 text-[10px] text-dim tracking-widest mb-2 px-1">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-accent opacity-70">
            <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
            <polyline points="17 21 17 13 7 13 7 21" />
            <polyline points="7 3 7 8 15 8" />
          </svg>
          {$_('ext.help.backupsTitle')}
        </div>
        <div class="border border-dotted border-border px-4 py-3 flex flex-col gap-2">
          <p class="text-sm text-muted leading-relaxed">
            {$_('ext.help.backupsExport')}
          </p>
          <p class="text-sm text-muted leading-relaxed">
            {$_('ext.help.backupsImport')}
          </p>
        </div>
      </div>

      <!-- Master password -->
      <div>
        <div class="flex items-center gap-2 text-[10px] text-dim tracking-widest mb-2 px-1">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-accent opacity-70">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
            <path d="M7 11V7a5 5 0 0 1 10 0v4" />
          </svg>
          {$_('ext.help.masterPasswordTitle')}
        </div>
        <div class="border border-dotted border-border px-4 py-3 flex flex-col gap-2">
          <p class="text-sm text-muted leading-relaxed">
            {$_('ext.help.masterPasswordDesc')}
          </p>
          <p class="text-xs text-dim leading-relaxed mt-1">
            {$_('ext.help.masterPasswordWarning')}
          </p>
        </div>
      </div>

      <!-- PIN & Recovery -->
      <div>
        <div class="flex items-center gap-2 text-[10px] text-dim tracking-widest mb-2 px-1">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-accent opacity-70">
            <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" />
          </svg>
          {$_('ext.help.pinRecoveryTitle')}
        </div>
        <div class="border border-dotted border-border px-4 py-3 flex flex-col gap-2">
          <p class="text-sm text-muted leading-relaxed">
            {$_('ext.help.pinRecoveryDesc')}
          </p>
          <p class="text-xs text-dim leading-relaxed mt-1">
            {$_('ext.help.pinRecoveryNote')}
          </p>
        </div>
      </div>

      <!-- Warning -->
      <div>
        <div class="flex items-center gap-2 text-[10px] text-dim tracking-widest mb-2 px-1">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-error opacity-70">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            <line x1="12" y1="9" x2="12" y2="13" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
          {$_('ext.help.losingAccessTitle')}
        </div>
        <div class="border border-dotted border-error/20 px-4 py-3">
          <p class="text-xs text-error/70 leading-relaxed">
            {$_('ext.help.losingAccessDesc')}
          </p>
        </div>
      </div>
    </div>
  </div>
</div>
