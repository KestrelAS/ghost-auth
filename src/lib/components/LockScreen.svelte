<script lang="ts">
  import ghostLogo from "$lib/assets/ghost.svg";
  import { _ } from 'svelte-i18n';
  import { verifyPin, verifyRecoveryCode, hasRecoveryCodes } from "$lib/stores/accounts";

  let { onunlock, biometricEnabled = false }: { onunlock: () => void; biometricEnabled?: boolean } = $props();

  let pin = $state("");
  let error = $state("");
  let shake = $state(false);
  let checking = $state(false);
  let biometricAvailable = $state(false);
  let biometricChecked = false;

  // Recovery mode
  let recoveryMode = $state(false);
  let recoveryCode = $state("");
  let hasRecovery = $state(false);
  let recoveryChecking = $state(false);

  const MAX_PIN = 8;

  // Wait until the app is fully visible (handles background-to-foreground
  // transitions on iOS where presenting Face ID too early freezes the UI).
  async function waitForVisible(): Promise<void> {
    if (!document.hidden) return;
    await new Promise<void>((resolve) => {
      function onVisible() {
        if (!document.hidden) {
          document.removeEventListener('visibilitychange', onVisible);
          resolve();
        }
      }
      document.addEventListener('visibilitychange', onVisible);
      setTimeout(() => {
        document.removeEventListener('visibilitychange', onVisible);
        resolve();
      }, 5000);
    });
    // Let iOS finish its app-switch animation before presenting Face ID
    await new Promise((r) => setTimeout(r, 600));
  }

  // Check biometric availability on mount
  async function checkBiometric() {
    if (biometricChecked) return;
    biometricChecked = true;

    if (!biometricEnabled) {
      biometricAvailable = false;
      return;
    }

    await waitForVisible();

    try {
      const { checkStatus } = await import("@tauri-apps/plugin-biometric");
      const status = await Promise.race([
        checkStatus(),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('timeout')), 3000)
        ),
      ]);
      biometricAvailable = status.isAvailable;
      if (biometricAvailable) {
        promptBiometric();
      }
    } catch {
      // Not available (desktop or plugin not supported)
      biometricAvailable = false;
    }
  }

  async function promptBiometric() {
    try {
      const { authenticate } = await import("@tauri-apps/plugin-biometric");
      await Promise.race([
        authenticate($_('lockScreen.biometricPrompt'), {
          allowDeviceCredential: false,
          confirmationRequired: false,
        }),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('timeout')), 15000)
        ),
      ]);
      onunlock();
    } catch (e) {
      const msg = String(e).toLowerCase();
      if (!msg.includes("cancel") && !msg.includes("user") && !msg.includes("timeout")) {
        error = $_('lockScreen.biometricFailed');
      }
    }
  }

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
      const ok = await verifyPin(pin);
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
        onunlock();
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
    checkBiometric();
    const delay = document.hidden ? 1000 : 0;
    setTimeout(() => {
      Promise.race([
        hasRecoveryCodes(),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('timeout')), 3000)
        ),
      ]).then((v) => { hasRecovery = v; }).catch(() => {});
    }, delay);
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 z-50 bg-bg flex flex-col items-center justify-center select-none pt-safe pb-safe">
  <!-- Ghost logo -->
  <img src={ghostLogo} alt="" class="w-20 h-20 icon-adapt opacity-40 mb-8" />

  {#if recoveryMode}
    <!-- Recovery code input -->
    <span class="text-base text-muted tracking-wide mb-6">{$_('lockScreen.recoveryCodeTitle')}</span>

    <input
      type="text"
      bind:value={recoveryCode}
      placeholder={$_('lockScreen.recoveryPlaceholder')}
      class="bg-transparent border border-dotted border-border text-fg text-center text-lg tracking-[0.3em] uppercase px-5 py-4 w-60 outline-none focus:border-fg/40 transition-colors placeholder:text-dim/40 {shake ? 'animate-shake' : ''}"
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
        class="text-sm text-dim border border-dotted border-border px-5 py-3 hover:text-fg hover:border-fg/30 transition-colors"
        onclick={() => { recoveryMode = false; recoveryCode = ""; error = ""; }}
      >
        {$_('common.back')}
      </button>
      <button
        type="button"
        class="text-sm px-5 py-3 transition-colors {recoveryCode.trim() ? 'text-fg border border-fg/80 hover:bg-fg hover:text-bg' : 'text-dim/50 border border-border cursor-default'}"
        onclick={submitRecovery}
        disabled={!recoveryCode.trim() || recoveryChecking}
      >
        {recoveryChecking ? $_('common.loading') : $_('lockScreen.verify')}
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
    <div class="grid grid-cols-3 gap-4">
      {#each ["1", "2", "3", "4", "5", "6", "7", "8", "9"] as digit}
        <button
          type="button"
          class="numpad-key w-20 h-20 flex items-center justify-center text-2xl text-fg/80 hover:text-fg rounded-lg"
          onclick={() => pressKey(digit)}
        >
          {digit}
        </button>
      {/each}
      <!-- Bottom row: biometric or del, 0, ok -->
      {#if biometricAvailable}
        <button
          type="button"
          class="numpad-key w-20 h-20 flex items-center justify-center text-dim hover:text-fg"
          onclick={promptBiometric}
          aria-label={$_('lockScreen.biometricAriaLabel')}
        >
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M7.5 11c0-2.5 2-4.5 4.5-4.5s4.5 2 4.5 4.5" />
            <path d="M12 12v4" />
            <path d="M5 10c0-3.9 3.1-7 7-7s7 3.1 7 7" />
            <path d="M3 9c0-5 4-9 9-9s9 4 9 9" />
            <path d="M10 12c0-1.1.9-2 2-2s2 .9 2 2v3c0 1.1-.9 2-2 2" />
            <path d="M7.5 15c0 2.5 2 4.5 4.5 4.5 1.4 0 2.6-.6 3.4-1.6" />
            <path d="M5 14c0 3.9 3.1 7 7 7 2.8 0 5.2-1.6 6.3-4" />
          </svg>
        </button>
      {:else}
        <button
          type="button"
          class="numpad-key w-20 h-20 flex items-center justify-center text-sm text-dim hover:text-fg"
          onclick={backspace}
        >
          {$_('numpad.del')}
        </button>
      {/if}
      <button
        type="button"
        class="numpad-key w-20 h-20 flex items-center justify-center text-2xl text-fg/80 hover:text-fg rounded-lg"
        onclick={() => pressKey("0")}
      >
        0
      </button>
      <button
        type="button"
        class="numpad-key w-20 h-20 flex items-center justify-center text-sm {pin.length >= 4 ? 'text-fg hover:text-fg/80' : 'text-dim/50 cursor-default'}"
        onclick={submit}
        disabled={pin.length < 4}
      >
        {$_('numpad.ok')}
      </button>
    </div>

    <!-- Forgot PIN -->
    {#if hasRecovery}
      <button
        type="button"
        class="mt-8 text-sm text-dim hover:text-fg transition-colors tracking-wider"
        onclick={() => { recoveryMode = true; error = ""; pin = ""; }}
      >
        {$_('lockScreen.forgotPin')}
      </button>
    {/if}
  {/if}
</div>
