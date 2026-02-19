<script lang="ts">
  import { editAccount, type AccountDisplay } from "$lib/stores/accounts.svelte";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { toast } from "$lib/stores/toast";
  import { inputClass, btnPrimary, btnSecondary } from "$lib/styles/styles";
  import { _ } from 'svelte-i18n';

  let {
    account,
    onclose,
    onsuccess,
  }: {
    account: AccountDisplay;
    onclose: () => void;
    onsuccess: () => void;
  } = $props();

  let issuer = $state(account.issuer);
  let label = $state(account.label);
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

  async function handleSave() {
    error = "";
    loading = true;
    try {
      await editAccount(account.id, issuer.trim(), label.trim());
      toast($_('editAccount.updated'));
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
    aria-labelledby="edit-account-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-5">
      <span id="edit-account-title" class="text-sm tracking-wide text-muted">{$_('editAccount.title')}</span>
      <button
        type="button"
        class="text-dim hover:text-fg transition-colors p-1"
        onclick={close}
        aria-label={$_('common.close')}
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <line x1="2" y1="2" x2="12" y2="12" /><line x1="12" y1="2" x2="2" y2="12" />
        </svg>
      </button>
    </div>

    {#if error}
      <div class="border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-xs">
        <span class="text-error/60">{$_('common.errorPrefix')}</span> {error}
      </div>
    {/if}

    <form
      class="flex flex-col gap-3"
      onsubmit={(e) => { e.preventDefault(); handleSave(); }}
    >
      <div>
        <label for="edit-issuer" class="block text-xs text-dim tracking-wide mb-1.5">{$_('editAccount.serviceLabel')}</label>
        <input
          id="edit-issuer"
          type="text"
          bind:value={issuer}
          placeholder={$_('editAccount.servicePlaceholder')}
          class={inputClass}
        />
      </div>
      <div>
        <label for="edit-label" class="block text-xs text-dim tracking-wide mb-1.5">{$_('editAccount.accountLabel')}</label>
        <input
          id="edit-label"
          type="text"
          bind:value={label}
          placeholder={$_('editAccount.accountPlaceholder')}
          class={inputClass}
        />
      </div>

      <div class="flex gap-2 mt-3">
        <button type="button" class={btnSecondary} onclick={close}>
          {$_('common.cancel')}
        </button>
        <button type="submit" disabled={loading} class="{btnPrimary} disabled:opacity-30">
          {loading ? $_('common.loading') : $_('common.save')}
        </button>
      </div>
    </form>
  </div>
</div>
