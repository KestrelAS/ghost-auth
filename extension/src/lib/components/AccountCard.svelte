<script lang="ts">
  import type { AccountDisplay, CodeResponse } from "$lib/stores/accounts.svelte";
  import CountdownRing from "./CountdownRing.svelte";
  import { toast } from "$lib/stores/toast";
  import { _ } from 'svelte-i18n';

  let {
    account,
    code,
    ondelete,
    onedit,
  }: {
    account: AccountDisplay;
    code: CodeResponse | undefined;
    ondelete: (id: string) => void;
    onedit: (account: AccountDisplay) => void;
  } = $props();

  let showActions = $state(false);
  let showConfirm = $state(false);

  function formatCode(raw: string): string {
    if (raw.length === 6) return `${raw.slice(0, 3)} ${raw.slice(3)}`;
    if (raw.length === 8) return `${raw.slice(0, 4)} ${raw.slice(4)}`;
    return raw;
  }

  async function copyCode() {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.code);
      toast($_('accountCard.copied'));
    } catch {
      // Silent fail
    }
  }

  function handleDelete() {
    if (!showConfirm) {
      showConfirm = true;
      return;
    }
    showConfirm = false;
    showActions = false;
    ondelete(account.id);
  }

  function cancelDelete() {
    showConfirm = false;
  }

  function handleCardKeydown(e: KeyboardEvent) {
    if (e.key === "e" || e.key === "E") {
      e.preventDefault();
      onedit(account);
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      handleDelete();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="relative py-3 group"
  onkeydown={handleCardKeydown}
  onmouseenter={() => showActions = true}
  onmouseleave={() => { showActions = false; showConfirm = false; }}
>
  <div class="flex items-center gap-3">
    <!-- Code area (clickable to copy) -->
    <button
      type="button"
      class="flex-1 min-w-0 text-left"
      onclick={copyCode}
    >
      <div class="mb-1">
        <span class="text-sm text-fg/85 truncate block">
          {account.issuer || $_('accountCard.unknown')}
        </span>
        {#if account.label}
          <span class="text-xs text-muted truncate block mt-0.5">
            {account.label}
          </span>
        {/if}
      </div>

      <div class="flex items-center gap-2">
        <span class="text-[1.75rem] leading-none font-light tracking-[0.15em] text-fg tabular-nums">
          {#if code}
            {formatCode(code.code)}
          {:else}
            <span class="text-dim">--- ---</span>
          {/if}
        </span>
      </div>
      {#if code}
        <span class="text-[9px] text-transparent group-hover:text-dim mt-0.5 block transition-colors duration-150 tracking-wider">
          {$_('accountCard.tapToCopy')}
        </span>
      {/if}
    </button>

    <!-- Countdown ring -->
    {#if code}
      <div class="flex-shrink-0">
        <CountdownRing remaining={code.remaining} period={account.period} />
      </div>
    {/if}
  </div>

  <!-- Hover action buttons -->
  {#if showActions}
    <div class="absolute top-1 right-0 flex gap-1">
      {#if !showConfirm}
        <button
          type="button"
          class="text-[10px] text-dim hover:text-fg px-1.5 py-0.5 transition-colors"
          onclick={() => onedit(account)}
        >
          {$_('accountCard.edit').toLowerCase()}
        </button>
        <button
          type="button"
          class="text-[10px] text-dim hover:text-error px-1.5 py-0.5 transition-colors"
          onclick={handleDelete}
        >
          {$_('accountCard.delete').toLowerCase()}
        </button>
      {:else}
        <span class="text-[10px] text-error/60 py-0.5">{$_('accountCard.confirmDelete').toLowerCase()}</span>
        <button
          type="button"
          class="text-[10px] text-error hover:text-error px-1 py-0.5 transition-colors"
          onclick={handleDelete}
        >
          {$_('accountCard.confirmYes')}
        </button>
        <button
          type="button"
          class="text-[10px] text-dim hover:text-fg px-1 py-0.5 transition-colors"
          onclick={cancelDelete}
        >
          {$_('accountCard.confirmNo')}
        </button>
      {/if}
    </div>
  {/if}
</div>
