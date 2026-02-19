<script lang="ts">
  import { _ } from 'svelte-i18n';
  import ghostLogo from "$lib/assets/ghost.svg";
  import type { AccountDisplay, CodeResponse } from "$lib/stores/accounts";
  import AccountCard from "./AccountCard.svelte";

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

  // Drag reorder state
  let draggingId: string | null = $state(null);
  let dropTargetIndex: number | null = $state(null);
  let dragActive = $state(false);
  let longPressTimer: ReturnType<typeof setTimeout> | null = null;
  let pressStartPos = { x: 0, y: 0 };
  let containerEl: HTMLDivElement | undefined = $state(undefined);

  const LONG_PRESS_MS = 500;
  const MOVE_THRESHOLD = 8;

  // Passive pointermove listener — required for iOS to initiate native
  // vertical scrolling without waiting for JS event processing.
  $effect(() => {
    if (!containerEl) return;
    containerEl.addEventListener('pointermove', handlePointerMove, { passive: true });
    return () => containerEl!.removeEventListener('pointermove', handlePointerMove);
  });

  function handlePointerDown(e: PointerEvent, accountId: string) {
    if (e.button !== 0) return;
    if (search.trim()) return;
    if (filtered.length < 2) return;

    pressStartPos = { x: e.clientX, y: e.clientY };

    longPressTimer = setTimeout(() => {
      activateDrag(accountId);
    }, LONG_PRESS_MS);
  }

  function handlePointerMove(e: PointerEvent) {
    if (longPressTimer) {
      const dx = Math.abs(e.clientX - pressStartPos.x);
      const dy = Math.abs(e.clientY - pressStartPos.y);
      if (dx > MOVE_THRESHOLD || dy > MOVE_THRESHOLD) {
        cancelLongPress();
      }
      return;
    }

    if (!dragActive) return;

    updateDropTarget(e.clientY);
  }

  function handlePointerUp() {
    cancelLongPress();
    if (dragActive) {
      commitDrag();
    }
  }

  function cancelLongPress() {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      longPressTimer = null;
    }
  }

  function activateDrag(accountId: string) {
    longPressTimer = null;
    draggingId = accountId;
    dragActive = true;
    if (containerEl) containerEl.style.touchAction = 'none';

    const sourceIndex = filtered.findIndex((a) => a.id === accountId);
    dropTargetIndex = sourceIndex;

    if (navigator.vibrate) {
      navigator.vibrate(50);
    }
  }

  function updateDropTarget(clientY: number) {
    if (!containerEl) return;
    const items = containerEl.querySelectorAll("[data-drag-item]");

    for (let i = 0; i < items.length; i++) {
      const rect = items[i].getBoundingClientRect();
      if (clientY < rect.top + rect.height / 2) {
        dropTargetIndex = i;
        return;
      }
    }
    dropTargetIndex = items.length - 1;
  }

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

    // Re-focus the moved item after DOM update
    requestAnimationFrame(() => {
      containerEl?.querySelectorAll<HTMLElement>("[data-drag-item]")[targetIndex]?.focus();
    });
  }

  async function commitDrag() {
    if (draggingId !== null && dropTargetIndex !== null) {
      const sourceIndex = filtered.findIndex((a) => a.id === draggingId);
      if (sourceIndex !== dropTargetIndex && sourceIndex !== -1) {
        const ids = accounts.map((a) => a.id);
        const fromGlobalIndex = accounts.findIndex((a) => a.id === draggingId);
        const targetAccount = filtered[dropTargetIndex];
        const toGlobalIndex = accounts.findIndex((a) => a.id === targetAccount?.id);

        if (fromGlobalIndex !== -1 && toGlobalIndex !== -1) {
          const [moved] = ids.splice(fromGlobalIndex, 1);
          ids.splice(toGlobalIndex, 0, moved);
          onreorder(ids);
        }
      }
    }

    draggingId = null;
    dropTargetIndex = null;
    dragActive = false;
    if (containerEl) containerEl.style.touchAction = 'pan-y';
  }
</script>

{#if accounts.length === 0}
  <div class="flex flex-col items-center justify-center py-24 text-dim">
    <img src={ghostLogo} alt="" class="w-20 h-20 icon-adapt opacity-15 mb-8" />
    <p class="text-base text-muted">{$_('accountList.emptyTitle')}</p>
    <p class="text-base text-dim mt-1">{$_('accountList.emptyHint')}</p>
  </div>
{:else if filtered.length === 0}
  <div class="flex flex-col items-center justify-center py-16 text-dim">
    <p class="text-xs text-muted">{$_('accountList.noMatches', { values: { search } })}</p>
  </div>
{:else}
  <div
    role="list"
    bind:this={containerEl}
    style="touch-action: pan-y;"
    onpointerup={handlePointerUp}
    onpointercancel={handlePointerUp}
  >
    {#each filtered as account, i (account.id)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
      <div
        data-drag-item
        role="listitem"
        tabindex="0"
        aria-label="{account.issuer || $_('accountList.accountFallback')}{account.label ? ` — ${account.label}` : ''}"
        style="touch-action: pan-y;"
        class="transition-all duration-150 outline-none focus-visible:ring-1 focus-visible:ring-accent
          {i > 0 ? 'border-dotted-t' : ''}
          {dragActive && draggingId === account.id ? 'drag-active-item' : ''}
          {dragActive && draggingId !== account.id && dropTargetIndex === i ? 'drag-drop-target' : ''}"
        onpointerdown={(e) => handlePointerDown(e, account.id)}
        onkeydown={(e) => handleKeyboardReorder(e, account.id)}
      >
        <AccountCard {account} code={codes.get(account.id)} {ondelete} {onedit} dragging={dragActive} />
      </div>
    {/each}
  </div>
{/if}

<style>
  .drag-active-item {
    transform: scale(1.02);
    opacity: 0.7;
    z-index: 10;
    position: relative;
  }

  .drag-drop-target {
    border-top: 2px solid var(--color-accent);
  }
</style>
