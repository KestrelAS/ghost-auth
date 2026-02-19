<script lang="ts">
  import { storage } from "$lib/stores/accounts.svelte";
  import { exportBackup } from "$core/backup";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { toast } from "$lib/stores/toast";
  import { inputClass, btnPrimary, btnSecondary } from "$lib/styles/styles";
  import { _ } from 'svelte-i18n';
  import iconPassword from "$lib/assets/icons/password.svg";

  let { onclose }: { onclose: () => void } = $props();

  let password = $state("");
  let confirm = $state("");
  let error = $state("");
  let loading = $state(false);

  let mounted = $state(false);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
  }

  async function handleExport() {
    error = "";
    if (password.length < 8) {
      error = $_('backupExport.passwordTooShort');
      return;
    }
    if (password !== confirm) {
      error = $_('backupExport.passwordMismatch');
      return;
    }
    loading = true;
    try {
      const accounts = await storage.getAccounts();
      const bytes = await exportBackup(accounts, password);

      // Trigger download
      const blob = new Blob([bytes], { type: "application/octet-stream" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `ghost-auth-backup-${Math.floor(Date.now() / 1000)}.ghostauth`;
      a.click();
      URL.revokeObjectURL(url);

      toast($_('backupExport.exported'));
      close();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
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
    aria-labelledby="export-backup-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-6">
      <span id="export-backup-title" class="text-base tracking-wide text-muted">{$_('backupExport.title')}</span>
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

    <p class="text-sm text-muted mb-4">
      {$_('backupExport.description')}
    </p>

    <form
      class="flex flex-col gap-3"
      onsubmit={(e) => { e.preventDefault(); handleExport(); }}
    >
      <div>
        <label for="backup-password" class="flex items-center gap-1.5 text-sm text-dim tracking-wide mb-1.5">
          <img src={iconPassword} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
          {$_('backupExport.passwordLabel')}
        </label>
        <input
          id="backup-password"
          type="password"
          bind:value={password}
          placeholder={$_('backupExport.passwordPlaceholder')}
          class={inputClass}
        />
      </div>
      <div>
        <label for="backup-confirm" class="flex items-center gap-1.5 text-sm text-dim tracking-wide mb-1.5">
          <img src={iconPassword} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
          {$_('backupExport.confirmLabel')}
        </label>
        <input
          id="backup-confirm"
          type="password"
          bind:value={confirm}
          placeholder={$_('backupExport.confirmPlaceholder')}
          class={inputClass}
        />
      </div>

      <div class="flex gap-2 mt-3">
        <button type="button" class={btnSecondary} onclick={close}>
          {$_('common.cancel')}
        </button>
        <button type="submit" disabled={loading} class="{btnPrimary} disabled:opacity-30">
          {loading ? $_('ext.setup.encrypting') : $_('common.export')}
        </button>
      </div>
    </form>
  </div>
</div>
