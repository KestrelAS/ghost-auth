<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { getExportAccounts, type ExportBatch } from "$lib/stores/accounts";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { getTheme } from "$lib/stores/theme.svelte";
  import QRCode from "qrcode";
  import ghostLogo from "$lib/assets/ghost.svg";

  let { onclose }: { onclose: () => void } = $props();

  let batches: ExportBatch[] = $state([]);
  let currentIndex = $state(0);
  let qrSvg = $state("");
  let error = $state("");
  let loading = $state(true);
  let mounted = $state(false);

  let current = $derived(batches[currentIndex]);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  $effect(() => {
    loadBatches();
  });

  $effect(() => {
    if (batches.length > 0) {
      generateQr(batches[currentIndex].migration_uri);
    }
  });

  async function loadBatches() {
    try {
      batches = await getExportAccounts();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function generateQr(uri: string) {
    try {
      qrSvg = await QRCode.toString(uri, {
        type: "svg",
        errorCorrectionLevel: "M",
        margin: 1,
        color: { dark: getTheme() === "dark" ? "#ffffff" : "#1a1a1a", light: "#00000000" },
      });
    } catch (e) {
      error = String(e);
    }
  }

  function prev() {
    if (currentIndex > 0) currentIndex--;
  }

  function next() {
    if (currentIndex < batches.length - 1) currentIndex++;
  }

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
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
    aria-labelledby="export-qr-title"
    tabindex="-1"
    use:trapFocus
  >
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <span id="export-qr-title" class="text-base tracking-wide text-muted">{$_('exportQr.title')}</span>
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

    {#if error}
      <div class="border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-sm">
        <span class="text-error/60">{$_('common.errorPrefix')}</span> {error}
      </div>
    {/if}

    {#if loading}
      <div class="text-center py-8">
        <p class="text-dim text-sm">{$_('common.loading')}</p>
      </div>
    {:else if batches.length === 0}
      <div class="text-center py-8">
        <p class="text-dim text-sm">{$_('exportQr.noAccounts')}</p>
      </div>
    {:else if current}
      <div class="flex flex-col items-center gap-4">
        <!-- QR Code -->
        {#if qrSvg}
          <div class="relative w-52 h-52">
            <div class="w-full h-full qr-container">
              {@html qrSvg}
            </div>
            <div class="absolute inset-0 flex items-center justify-center">
              <div class="w-11 h-11 bg-bg rounded-sm flex items-center justify-center p-1.5">
                <img src={ghostLogo} alt="" class="w-full h-full icon-adapt opacity-60" />
              </div>
            </div>
          </div>
        {/if}

        <!-- Accounts in this batch -->
        <div class="w-full border border-dotted border-border px-4 py-3">
          <div class="text-xs text-dim tracking-wide mb-2">
            {$_('exportQr.accountsInBatch', { values: { count: current.accounts.length } })}
          </div>
          {#each current.accounts as account}
            <div class="py-1">
              <span class="text-sm text-fg">{account.issuer || account.label}</span>
              {#if account.issuer && account.label}
                <span class="text-xs text-dim"> — {account.label}</span>
              {/if}
            </div>
          {/each}
        </div>

        <p class="text-xs text-dim text-center">{$_('exportQr.description')}</p>

        <!-- Navigation (only show if multiple batches) -->
        {#if batches.length > 1}
          <div class="w-full flex items-center justify-between mt-2">
            <button
              type="button"
              class="border border-dotted border-border text-dim text-sm px-4 py-2 hover:text-fg hover:border-fg/30 transition-colors disabled:opacity-20 disabled:pointer-events-none"
              disabled={currentIndex === 0}
              onclick={prev}
              aria-label={$_('common.back')}
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="9 2 4 7 9 12" />
              </svg>
            </button>
            <span class="text-sm text-dim">
              {$_('exportQr.counter', { values: { current: currentIndex + 1, total: batches.length } })}
            </span>
            <button
              type="button"
              class="border border-dotted border-border text-dim text-sm px-4 py-2 hover:text-fg hover:border-fg/30 transition-colors disabled:opacity-20 disabled:pointer-events-none"
              disabled={currentIndex === batches.length - 1}
              onclick={next}
              aria-label="Next"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="5 2 10 7 5 12" />
              </svg>
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .qr-container :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
