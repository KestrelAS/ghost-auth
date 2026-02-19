<script lang="ts">
  import { addAccountFromUri, addAccountManual } from "$lib/stores/accounts.svelte";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { inputClass, btnPrimary, btnSecondary } from "$lib/styles/styles";
  import { _ } from 'svelte-i18n';
  import iconManualEntry from "$lib/assets/icons/manual-entry.svg";
  import iconPaste from "$lib/assets/icons/paste.svg";

  let { onclose, onsuccess }: {
    onclose: () => void;
    onsuccess: () => void;
  } = $props();

  let mode: "choose" | "manual" | "uri" = $state("choose");
  let error = $state("");
  let loading = $state(false);

  // Manual entry fields
  let issuer = $state("");
  let label = $state("");
  let secret = $state("");
  // URI entry
  let uri = $state("");

  async function submitManual() {
    error = "";
    if (!secret.trim()) {
      error = $_('addAccount.secretRequired');
      return;
    }
    loading = true;
    try {
      await addAccountManual(
        issuer.trim(),
        label.trim(),
        secret.trim(),
        "SHA1",
        6,
        30,
      );
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function submitUri() {
    error = "";
    if (!uri.trim()) {
      error = $_('addAccount.uriRequired');
      return;
    }
    loading = true;
    try {
      await addAccountFromUri(uri.trim());
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

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
    aria-labelledby="add-account-title"
    tabindex="-1"
    use:trapFocus
  >
    <!-- Header -->
    <div class="flex items-center justify-between mb-5">
      <span id="add-account-title" class="text-base tracking-wide text-muted">{$_('addAccount.title')}</span>
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

    {#if mode === "choose"}
      <div class="flex flex-col gap-2">
        <button
          type="button"
          class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
          onclick={() => (mode = "manual")}
        >
          <div class="text-base text-fg group-hover:text-fg flex items-center gap-2">
            <img src={iconManualEntry} alt="" class="w-4 h-4 icon-adapt opacity-60" />
            {$_('addAccount.manualEntry')}
          </div>
          <div class="text-sm text-dim mt-1 ml-6">{$_('addAccount.manualEntryDesc')}</div>
        </button>
        <button
          type="button"
          class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
          onclick={() => (mode = "uri")}
        >
          <div class="text-base text-fg group-hover:text-fg flex items-center gap-2">
            <img src={iconPaste} alt="" class="w-4 h-4 icon-adapt opacity-60" />
            {$_('addAccount.pasteUri')}
          </div>
          <div class="text-sm text-dim mt-1 ml-6">{$_('addAccount.pasteUriDesc')}</div>
        </button>
      </div>

    {:else if mode === "manual"}
      <form
        class="flex flex-col gap-3"
        onsubmit={(e) => { e.preventDefault(); submitManual(); }}
      >
        <div>
          <label for="issuer" class="block text-sm text-dim tracking-wide mb-1.5">{$_('addAccount.serviceLabel')}</label>
          <input
            id="issuer"
            type="text"
            bind:value={issuer}
            placeholder={$_('addAccount.servicePlaceholder')}
            class={inputClass}
          />
        </div>
        <div>
          <label for="label" class="block text-sm text-dim tracking-wide mb-1.5">{$_('addAccount.accountLabel')}</label>
          <input
            id="label"
            type="text"
            bind:value={label}
            placeholder={$_('addAccount.accountPlaceholder')}
            class={inputClass}
          />
        </div>
        <div>
          <label for="secret" class="block text-sm text-dim tracking-wide mb-1.5">{$_('addAccount.secretKeyLabel')} <span class="text-dim">{$_('addAccount.secretKeyRequired')}</span></label>
          <input
            id="secret"
            type="text"
            bind:value={secret}
            placeholder={$_('addAccount.secretKeyPlaceholder')}
            class="{inputClass} uppercase"
          />
        </div>

        <div class="flex gap-2 mt-3">
          <button type="button" class={btnSecondary} onclick={() => (mode = "choose")}>
            {$_('common.back')}
          </button>
          <button type="submit" disabled={loading} class="{btnPrimary} disabled:opacity-30">
            {loading ? $_('common.loading') : $_('common.add')}
          </button>
        </div>
      </form>

    {:else if mode === "uri"}
      <form
        class="flex flex-col gap-3"
        onsubmit={(e) => { e.preventDefault(); submitUri(); }}
      >
        <div>
          <label for="uri" class="block text-sm text-dim tracking-wide mb-1.5">{$_('addAccount.uriLabel')}</label>
          <textarea
            id="uri"
            bind:value={uri}
            placeholder={$_('addAccount.uriPlaceholder')}
            rows="3"
            class="{inputClass} resize-none"
          ></textarea>
        </div>
        <div class="flex gap-2 mt-1">
          <button type="button" class={btnSecondary} onclick={() => (mode = "choose")}>
            {$_('common.back')}
          </button>
          <button type="submit" disabled={loading} class="{btnPrimary} disabled:opacity-30">
            {loading ? $_('common.loading') : $_('common.add')}
          </button>
        </div>
      </form>
    {/if}
  </div>
</div>
