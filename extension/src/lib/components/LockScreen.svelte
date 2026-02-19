<script lang="ts">
  import ghostLogo from "$lib/assets/ghost.svg";
  import { verifyPin, verifyRecoveryCode, hasRecoveryCodes } from "$core/pin";
  import { _ } from 'svelte-i18n';

  let { onunlock, onsubmitpin, onpasswordfallback, onrecoveryused }: {
    onunlock: () => void;
    onsubmitpin?: (pin: string) => Promise<boolean>;
    onpasswordfallback?: () => void;
    onrecoveryused?: () => void;
  } = $props();

  let pin = $state("");
  let error = $state("");
  let shake = $state(false);
  let checking = $state(false);

  // Recovery mode
  let recoveryMode = $state(false);
  let recoveryCode = $state("");
  let hasRecovery = $state(false);
  let recoveryChecking = $state(false);

  const MAX_PIN = 8;

  function pressKey(digit: string) {
    if (pin.length >= MAX_PIN || checking) return;
    pin += digit;
    error = "";
  }

  function backspace() {
    if (checking) return;
    pin = pin.slice(0, -1);
    error = "";
  }

  async function submit() {
    if (pin.length < 4 || checking) return;
    checking = true;
    try {
      const ok = onsubmitpin
        ? await onsubmitpin(pin)
        : await verifyPin(pin);
      if (ok) {
        onunlock();
      } else {
        error = $_('lockScreen.incorrectPin');
        shake = true;
        setTimeout(() => { shake = false; }, 500);
        pin = "";
      }
    } catch (e) {
      const msg = String(e);
      if (msg.includes("Too many attempts")) {
        error = msg.toLowerCase();
      } else {
        error = $_('lockScreen.verificationFailed');
      }
      pin = "";
    } finally {
      checking = false;
    }
  }

  async function submitRecovery() {
    if (!recoveryCode.trim() || recoveryChecking) return;
    recoveryChecking = true;
    try {
      const ok = await verifyRecoveryCode(recoveryCode.trim());
      if (ok) {
        // Recovery code removes PIN — in cold mode, redirect to master password
        if (onrecoveryused) {
          onrecoveryused();
        } else {
          onunlock();
        }
      } else {
        error = $_('lockScreen.invalidRecovery');
        shake = true;
        setTimeout(() => { shake = false; }, 500);
        recoveryCode = "";
      }
    } catch (e) {
      const msg = String(e);
      if (msg.includes("Too many attempts")) {
        error = msg.toLowerCase();
      } else {
        error = $_('lockScreen.verificationFailed');
      }
      recoveryCode = "";
    } finally {
      recoveryChecking = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (recoveryMode) {
      if (e.key === "Enter") submitRecovery();
      if (e.key === "Escape") {
        recoveryMode = false;
        recoveryCode = "";
        error = "";
      }
      return;
    }
    if (e.key >= "0" && e.key <= "9") {
      pressKey(e.key);
    } else if (e.key === "Backspace") {
      backspace();
    } else if (e.key === "Enter") {
      submit();
    }
  }

  $effect(() => {
    hasRecoveryCodes().then((v) => { hasRecovery = v; }).catch(() => {});
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 z-50 bg-bg flex flex-col items-center justify-center select-none">
  <!-- Ghost logo -->
  <img src={ghostLogo} alt="" class="w-16 h-16 icon-adapt opacity-40 mb-6" />

  {#if recoveryMode}
    <!-- Recovery code input -->
    <span class="text-base text-muted tracking-wide mb-6">{$_('lockScreen.recoveryCodeTitle')}</span>

    <input
      type="text"
      bind:value={recoveryCode}
      placeholder={$_('lockScreen.recoveryPlaceholder')}
      class="bg-transparent border border-dotted border-border text-fg text-center text-lg tracking-[0.3em] uppercase px-5 py-3 w-56 outline-none focus:border-fg/40 transition-colors placeholder:text-dim/40 {shake ? 'animate-shake' : ''}"
      disabled={recoveryChecking}
    />

    <div class="h-6 mt-3 mb-6">
      {#if error}
        <span class="text-xs text-error">{error}</span>
      {/if}
    </div>

    <div class="flex gap-3">
      <button
        type="button"
        class="text-sm text-dim border border-dotted border-border px-5 py-2.5 hover:text-fg hover:border-fg/30 transition-colors"
        onclick={() => { recoveryMode = false; recoveryCode = ""; error = ""; }}
      >
        {$_('common.back')}
      </button>
      <button
        type="button"
        class="text-sm px-5 py-2.5 transition-colors {recoveryCode.trim() ? 'text-fg border border-fg/80 hover:bg-fg hover:text-bg' : 'text-dim/50 border border-border cursor-default'}"
        onclick={submitRecovery}
        disabled={!recoveryCode.trim() || recoveryChecking}
      >
        {recoveryChecking ? '...' : $_('lockScreen.verify')}
      </button>
    </div>
  {:else}
    <!-- PIN dots -->
    <div class="flex gap-2.5 mb-3 h-8 items-center justify-center {shake ? 'animate-shake' : ''}">
      {#each { length: pin.length } as _}
        <div class="w-3 h-3 rounded-full bg-fg pin-dot-filled"></div>
      {/each}
    </div>

    <!-- Error message -->
    <div class="h-6 mb-6">
      {#if error}
        <span class="text-xs text-error">{error}</span>
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

    <!-- Forgot PIN / Fallback -->
    <div class="flex flex-col items-center gap-2 mt-6">
      {#if hasRecovery}
        <button
          type="button"
          class="text-sm text-dim hover:text-fg transition-colors tracking-wider"
          onclick={() => { recoveryMode = true; error = ""; pin = ""; }}
        >
          {$_('lockScreen.forgotPin').toLowerCase()}
        </button>
      {/if}
      {#if onpasswordfallback}
        <button
          type="button"
          class="text-sm text-dim hover:text-fg transition-colors tracking-wider"
          onclick={onpasswordfallback}
        >
          {$_('ext.unlock.useMasterPassword')}
        </button>
      {/if}
    </div>
  {/if}
</div>
