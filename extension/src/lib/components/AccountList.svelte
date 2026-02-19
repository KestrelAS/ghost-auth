<script lang="ts">
  import ghostLogo from "$lib/assets/ghost.svg";
  import type { AccountDisplay, CodeResponse } from "$lib/stores/accounts.svelte";
  import AccountCard from "./AccountCard.svelte";
  import { _ } from 'svelte-i18n';

  let {
    accounts,
    codes,
    ondelete,
    onedit,
    onreorder,
    search = "",
  }: {
    accounts: AccountDisplay[];
    codes: Map<string, CodeResponse>;
    ondelete: (id: string) => void;
    onedit: (account: AccountDisplay) => void;
    onreorder: (ids: string[]) => void;
    search?: string;
  } = $props();

  let filtered = $derived(
    search.trim()
      ? accounts.filter((a) => {
          const q = search.toLowerCase();
          return (
            a.issuer.toLowerCase().includes(q) ||
            a.label.toLowerCase().includes(q)
          );
        })
      : accounts,
  );

  function handleKeyboardReorder(e: KeyboardEvent, accountId: string) {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    if (search.trim()) return;
    if (accounts.length < 2) return;

    e.preventDefault();
    const ids = accounts.map((a) => a.id);
    const index = ids.indexOf(accountId);
    if (index === -1) return;

    const targetIndex = e.key === "ArrowUp" ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= ids.length) return;

    const [moved] = ids.splice(index, 1);
    ids.splice(targetIndex, 0, moved);
    onreorder(ids);
  }
</script>

{#if accounts.length === 0}
  <div class="flex flex-col items-center justify-center py-16 text-dim">
    <img src={ghostLogo} alt="" class="w-16 h-16 icon-adapt opacity-15 mb-6" />
    <p class="text-sm text-muted">{$_('accountList.emptyTitle')}</p>
    <p class="text-sm text-dim mt-1">{$_('ext.accountList.emptyHint')}</p>
  </div>
{:else if filtered.length === 0}
  <div class="flex flex-col items-center justify-center py-12 text-dim">
    <p class="text-xs text-muted">{$_('accountList.noMatches', { values: { search } })}</p>
  </div>
{:else}
  <div role="list">
    {#each filtered as account, i (account.id)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
      <div
        role="listitem"
        tabindex="0"
        aria-label="{account.issuer || $_('accountList.accountFallback')}{account.label ? ` — ${account.label}` : ''}"
        class="outline-none focus-visible:ring-1 focus-visible:ring-accent
          {i > 0 ? 'border-dotted-t' : ''}"
        onkeydown={(e) => handleKeyboardReorder(e, account.id)}
      >
        <AccountCard {account} code={codes.get(account.id)} {ondelete} {onedit} />
      </div>
    {/each}
  </div>
{/if}
