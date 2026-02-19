<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { importBackupPreview, importBackupConfirm, type BackupPreview } from "$lib/stores/accounts";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { toast } from "$lib/stores/toast";
  import { inputClass, btnPrimary, btnSecondary } from "$lib/styles";
  import iconFile from "$lib/assets/icons/file.svg";
  import iconPassword from "$lib/assets/icons/password.svg";

  let { onclose, onsuccess }: { onclose: () => void; onsuccess: () => void } = $props();

  let fileData: number[] | null = $state(null);
  let fileName = $state("");
  let password = $state("");
  let error = $state("");
  let loading = $state(false);
  let preview: BackupPreview | null = $state(null);

  let mounted = $state(false);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
  }

  function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    fileName = file.name;
    const reader = new FileReader();
    reader.onload = () => {
      fileData = Array.from(new Uint8Array(reader.result as ArrayBuffer));
    };
    reader.readAsArrayBuffer(file);
  }

  async function handlePreview() {
    if (!fileData || !password) return;
    error = "";
    loading = true;
    try {
      preview = await importBackupPreview(fileData, password);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function handleConfirm() {
    if (!fileData) return;
    loading = true;
    error = "";
    try {
      const added = await importBackupConfirm(fileData, password);
      toast($_('backupImport.imported', { values: { count: added.length } }));
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
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
    aria-labelledby="import-backup-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-6">
      <span id="import-backup-title" class="text-base tracking-wide text-muted">{$_('backupImport.title')}</span>
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

    {#if !preview}
      <form
        class="flex flex-col gap-3"
        onsubmit={(e) => { e.preventDefault(); handlePreview(); }}
      >
        <div>
          <label for="backup-file" class="flex items-center gap-1.5 text-sm text-dim tracking-wide mb-1.5">
            <img src={iconFile} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
            {$_('backupImport.fileLabel')}
          </label>
          <label class="block border border-dotted border-border px-3 py-2.5 text-base text-dim hover:border-fg/30 transition-colors cursor-pointer">
            {fileName || $_('backupImport.filePlaceholder')}
            <input
              id="backup-file"
              type="file"
              accept=".ghostauth"
              class="hidden"
              onchange={handleFileSelect}
            />
          </label>
        </div>
        <div>
          <label for="import-password" class="flex items-center gap-1.5 text-sm text-dim tracking-wide mb-1.5">
            <img src={iconPassword} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
            {$_('backupImport.passwordLabel')}
          </label>
          <input
            id="import-password"
            type="password"
            bind:value={password}
            placeholder={$_('backupImport.passwordPlaceholder')}
            class={inputClass}
          />
        </div>

        <div class="flex gap-2 mt-3">
          <button type="button" class={btnSecondary} onclick={close}>
            {$_('common.cancel')}
          </button>
          <button type="submit" disabled={loading || !fileData || !password} class="{btnPrimary} disabled:opacity-30">
            {loading ? $_('common.loading') : $_('backupImport.decrypt')}
          </button>
        </div>
      </form>
    {:else}
      <div class="mb-4">
        <p class="text-sm text-muted mb-3">
          {$_('backupImport.accountsFound', { values: { total: preview.accounts.length + preview.duplicates } })}{#if preview.duplicates > 0}{$_('backupImport.duplicatesExist', { values: { count: preview.duplicates } })}{/if}.
        </p>
        {#if preview.accounts.length === 0}
          <p class="text-sm text-dim">{$_('backupImport.allExist')}</p>
        {:else}
          <div class="flex flex-col gap-1 max-h-48 overflow-y-auto">
            {#each preview.accounts as account}
              <div class="border border-dotted border-border px-4 py-2.5">
                <div class="text-sm text-fg">{account.issuer}</div>
                <div class="text-xs text-dim">{account.label}</div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex gap-2">
        <button type="button" class={btnSecondary} onclick={() => { preview = null; error = ""; }}>
          {$_('common.back')}
        </button>
        <button type="button" disabled={loading || preview.accounts.length === 0} class="{btnPrimary} disabled:opacity-30" onclick={handleConfirm}>
          {loading ? $_('common.loading') : $_('common.import')}
        </button>
      </div>
    {/if}
  </div>
</div>
