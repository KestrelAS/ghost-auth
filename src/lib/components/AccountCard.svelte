<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";
  import type { AccountDisplay, CodeResponse } from "$lib/stores/accounts";
  import CountdownRing from "./CountdownRing.svelte";
  import { toast } from "$lib/stores/toast";

  let {
    account,
    code,
    ondelete,
    onedit,
    dragging = false,
  }: {
    account: AccountDisplay;
    code: CodeResponse | undefined;
    ondelete: (id: string) => void;
    onedit: (account: AccountDisplay) => void;
    dragging?: boolean;
  } = $props();

  let showConfirm = $state(false);

  // Swipe state
  let offsetX = $state(0);
  let swiping = $state(false);
  let snapping = $state(false);
  let swiped = $state(false);
  let startX = 0;
  let startY = 0;
  let locked = false; // locked to horizontal once determined
  let clipboardTimer: ReturnType<typeof setTimeout> | null = null;
  let activePointerId: number | null = null;
  let cardEl: HTMLDivElement | undefined = $state(undefined);

  const ACTION_WIDTH = 140;
  const SNAP_THRESHOLD = 50;
  const CLIPBOARD_CLEAR_DELAY = 30_000;

  // Passive pointermove listener — required for iOS to initiate native
  // vertical scrolling without waiting for JS event processing.
  $effect(() => {
    if (!cardEl) return;
    cardEl.addEventListener('pointermove', onPointerMove, { passive: true });
    return () => cardEl!.removeEventListener('pointermove', onPointerMove);
  });

  function formatCode(raw: string): string {
    if (raw.length === 6) return `${raw.slice(0, 3)} ${raw.slice(3)}`;
    if (raw.length === 8) return `${raw.slice(0, 4)} ${raw.slice(4)}`;
    return raw;
  }

  async function copyCode() {
    if (dragging) return;
    if (swiped || Math.abs(offsetX) > 4) return;
    if (!code) return;
    try {
      const copiedCode = code.code;
      await writeText(copiedCode);
      if (navigator.vibrate) navigator.vibrate(30);
      toast($_('accountCard.copied'));

      // Clear clipboard after 30 seconds if it still contains the code
      if (clipboardTimer) clearTimeout(clipboardTimer);
      clipboardTimer = setTimeout(async () => {
        try {
          const current = await readText();
          if (current === copiedCode) {
            await writeText("");
          }
        } catch {
          toast($_('accountCard.clipboardClearFailed'));
        }
      }, CLIPBOARD_CLEAR_DELAY);
    } catch {
      // Silent fail
    }
  }

  function onPointerDown(e: PointerEvent) {
    if (dragging) return;
    if (snapping) return;
    if (e.button !== 0) return;
    startX = e.clientX;
    startY = e.clientY;
    swiping = true;
    locked = false;
    snapping = false;
    activePointerId = e.pointerId;
  }

  function onPointerMove(e: PointerEvent) {
    if (!swiping || e.pointerId !== activePointerId) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;

    // Determine direction lock on first significant movement
    if (!locked && (Math.abs(dx) > 8 || Math.abs(dy) > 8)) {
      if (Math.abs(dy) > Math.abs(dx)) {
        // Vertical scroll — bail out so native scroll takes over
        swiping = false;
        try { cardEl?.releasePointerCapture(e.pointerId); } catch {}
        return;
      }
      locked = true;
      cardEl?.setPointerCapture(e.pointerId);
    }

    if (!locked) return;

    // Note: no e.preventDefault() needed — touch-action: pan-y on the element
    // already prevents vertical scroll during horizontal swipes, and keeping
    // this listener passive is required for iOS native scroll initiation.

    let target: number;
    if (swiped) {
      // Already open — allow swiping back closed
      target = -ACTION_WIDTH + dx;
    } else {
      target = dx;
    }

    // Clamp: no right overswipe, elastic resistance on left overswipe
    if (target > 0) {
      target = target * 0.2;
    } else if (target < -ACTION_WIDTH) {
      const over = -target - ACTION_WIDTH;
      target = -(ACTION_WIDTH + over * 0.2);
    }

    offsetX = target;
  }

  function onPointerUp(e: PointerEvent) {
    if (!swiping || e.pointerId !== activePointerId) return;
    activePointerId = null;
    swiping = false;
    snapping = true;

    if (swiped) {
      // Was open — close if swiped right past threshold
      if (offsetX > -ACTION_WIDTH + SNAP_THRESHOLD) {
        snapTo(0);
        swiped = false;
      } else {
        snapTo(-ACTION_WIDTH);
      }
    } else {
      // Was closed — open if swiped left past threshold
      if (offsetX < -SNAP_THRESHOLD) {
        snapTo(-ACTION_WIDTH);
        swiped = true;
      } else {
        snapTo(0);
      }
    }
  }

  function snapTo(target: number) {
    snapping = true;
    offsetX = target;
    setTimeout(() => { snapping = false; }, 300);
  }

  function closeSwipe() {
    swiped = false;
    snapTo(0);
  }

  function handleDelete() {
    if (!showConfirm) {
      showConfirm = true;
      return;
    }
    showConfirm = false;
    closeSwipe();
    ondelete(account.id);
  }

  function cancelDelete() {
    showConfirm = false;
  }

  function handleCardKeydown(e: KeyboardEvent) {
    if (e.key === "e" || e.key === "E") {
      e.preventDefault();
      closeSwipe();
      onedit(account);
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      handleDelete();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overflow-hidden relative" onkeydown={handleCardKeydown}>
  <span class="sr-only">{$_('accountCard.srHint')}</span>
  <!-- Action buttons revealed behind -->
  <div
    class="absolute right-0 top-0 bottom-0 flex"
    style="width: {ACTION_WIDTH}px;"
  >
    {#if !showConfirm}
      <button
        type="button"
        class="flex-1 flex items-center justify-center text-[11px] text-muted bg-fg/3 hover:bg-fg/8 transition-colors border-l border-dotted border-border"
        onclick={() => { closeSwipe(); onedit(account); }}
      >
        {$_('accountCard.edit')}
      </button>
      <button
        type="button"
        class="flex-1 flex items-center justify-center text-[11px] text-error bg-error/8 hover:bg-error/15 transition-colors border-l border-dotted border-error/30"
        onclick={handleDelete}
      >
        {$_('accountCard.delete')}
      </button>
    {:else}
      <div class="flex-1 flex items-center justify-center gap-3 bg-error/8 border-l border-dotted border-error/30">
        <span class="text-[10px] text-error/60">{$_('accountCard.confirmDelete')}</span>
        <button
          type="button"
          class="text-[11px] text-error hover:text-error transition-colors"
          onclick={handleDelete}
        >
          {$_('accountCard.confirmYes')}
        </button>
        <button
          type="button"
          class="text-[11px] text-dim hover:text-fg transition-colors"
          onclick={cancelDelete}
        >
          {$_('accountCard.confirmNo')}
        </button>
      </div>
    {/if}
  </div>

  <!-- Sliding card content -->
  <div
    bind:this={cardEl}
    class="relative bg-bg py-4 {snapping ? 'swipe-snap' : ''}"
    style="transform: translateX({offsetX}px); touch-action: pan-y;"
    onpointerdown={onPointerDown}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    role="group"
  >
    <div class="flex items-center gap-4">
      <!-- Code area (tappable to copy) -->
      <button
        type="button"
        class="flex-1 min-w-0 text-left group"
        onclick={copyCode}
      >
        <div class="mb-1.5">
          <span class="text-base text-fg/85 truncate block">
            {account.issuer || $_('accountCard.unknown')}
          </span>
          {#if account.label}
            <span class="text-xs text-muted truncate block mt-0.5">
              {account.label}
            </span>
          {/if}
        </div>

        <div class="flex items-center gap-2.5">
          <span class="text-[2rem] leading-none font-light tracking-[0.15em] text-fg tabular-nums">
            {#if code}
              {formatCode(code.code)}
            {:else}
              <span class="text-dim">{$_('accountCard.codePlaceholder')}</span>
            {/if}
          </span>
        </div>
        {#if code}
          <span class="copy-hint text-[9px] text-transparent group-hover:text-dim mt-1 block transition-colors duration-150 tracking-wider">
            {$_('accountCard.tapToCopy')}
          </span>
        {/if}
      </button>

      <!-- Countdown ring -->
      {#if code}
        <div class="flex-shrink-0 pr-2">
          <CountdownRing remaining={code.remaining} period={account.period} />
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .swipe-snap {
    transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @media (hover: none) {
    .copy-hint {
      display: none;
    }
  }
</style>
