<script lang="ts">
  import { setPin } from "$core/pin";
  import RecoveryCodes from "./RecoveryCodes.svelte";
  import { _ } from 'svelte-i18n';

  let { onclose, ondone, onwrapdek }: {
    onclose: () => void;
    ondone: () => void;
    onwrapdek?: (pin: string) => Promise<void>;
  } = $props();

  let step: "enter" | "confirm" | "recovery" = $state("enter");
  let pin = $state("");
  let firstPin = $state("");
  let error = $state("");
  let shake = $state(false);
  let saving = $state(false);
  let recoveryCodes: string[] = $state([]);

  const MAX_PIN = 8;

  let prompt = $derived(step === "enter" ? $_('pinSetup.choosePin') : $_('pinSetup.confirmPin'));

  function pressKey(digit: string) {
    if (pin.length >= MAX_PIN || saving) return;
    pin += digit;
    error = "";
  }

  function backspace() {
    if (saving) return;
    pin = pin.slice(0, -1);
    error = "";
  }

  async function submit() {
    if (pin.length < 4 || saving) return;

    if (step === "enter") {
      firstPin = pin;
      pin = "";
      step = "confirm";
      return;
    }

    // Confirm step
    if (pin !== firstPin) {
      error = $_('pinSetup.mismatch');
      shake = true;
      setTimeout(() => { shake = false; }, 500);
      pin = "";
      step = "enter";
      firstPin = "";
      return;
    }

    saving = true;
    try {
      recoveryCodes = await setPin(pin);
      await onwrapdek?.(pin);
      step = "recovery";
    } catch (e) {
      error = String(e);
      pin = "";
      step = "enter";
      firstPin = "";
    } finally {
      saving = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (step === "recovery") return;
    if (e.key >= "0" && e.key <= "9") {
      pressKey(e.key);
    } else if (e.key === "Backspace") {
      backspace();
    } else if (e.key === "Enter") {
      submit();
    } else if (e.key === "Escape") {
      onclose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if step === "recovery"}
  <RecoveryCodes codes={recoveryCodes} {ondone} />
{:else}
  <div class="fixed inset-0 z-50 bg-bg flex flex-col items-center justify-center select-none">
    <!-- Title -->
    <span class="text-base text-muted tracking-wide mb-8">{prompt}</span>

    <!-- PIN dots -->
    <div class="flex gap-2.5 mb-3 h-8 items-center justify-center {shake ? 'animate-shake' : ''}">
      {#each { length: pin.length } as _}
        <div class="w-3 h-3 rounded-full bg-fg pin-dot-filled"></div>
      {/each}
    </div>

    <!-- Error / hint -->
    <div class="h-6 mb-6">
      {#if error}
        <span class="text-xs text-error">{error}</span>
      {:else}
        <span class="text-xs text-dim">{$_('pinSetup.digitHint')}</span>
      {/if}
    </div>

    <!-- Numpad -->
    <div class="grid grid-cols-3 gap-3">
      {#each ["1", "2", "3", "4", "5", "6", "7", "8", "9"] as digit}
        <button
          type="button"
          class="numpad-key w-16 h-16 flex items-center justify-center text-xl text-fg/80 hover:text-fg rounded-lg"
          onclick={() => pressKey(digit)}
        >
          {digit}
        </button>
      {/each}
      <button
        type="button"
        class="numpad-key w-16 h-16 flex items-center justify-center text-sm text-dim hover:text-fg"
        onclick={backspace}
      >
        {$_('numpad.del').toLowerCase()}
      </button>
      <button
        type="button"
        class="numpad-key w-16 h-16 flex items-center justify-center text-xl text-fg/80 hover:text-fg rounded-lg"
        onclick={() => pressKey("0")}
      >
        0
      </button>
      <button
        type="button"
        class="numpad-key w-16 h-16 flex items-center justify-center text-sm {pin.length >= 4 ? 'text-fg hover:text-fg/80' : 'text-dim/50 cursor-default'}"
        onclick={submit}
        disabled={pin.length < 4}
      >
        {$_('numpad.ok').toLowerCase()}
      </button>
    </div>

    <!-- Cancel -->
    <button
      type="button"
      class="mt-6 text-dim hover:text-fg transition-colors p-2"
      onclick={onclose}
      aria-label={$_('common.cancel')}
    >
      <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <line x1="2" y1="2" x2="12" y2="12" /><line x1="12" y1="2" x2="2" y2="12" />
      </svg>
    </button>
  </div>
{/if}
