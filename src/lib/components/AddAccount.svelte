<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { scan, cancel, Format, checkPermissions, requestPermissions, openAppSettings } from "@tauri-apps/plugin-barcode-scanner";
  import { addAccount, addAccountManual } from "$lib/stores/accounts";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { inputClass, btnPrimary, btnSecondary } from "$lib/styles";
  import WebScanner from "./WebScanner.svelte";
  import iconQr from "$lib/assets/icons/qr.svg";
  import iconManualEntry from "$lib/assets/icons/manual-entry.svg";
  import iconPaste from "$lib/assets/icons/paste.svg";
  import iconApp from "$lib/assets/icons/app.svg";

  let { onclose, onsuccess, onmigration, onimportexternal, onscanstart, onscanend }: {
    onclose: () => void;
    onsuccess: () => void;
    onmigration: (data: number[]) => void;
    onimportexternal: () => void;
    onscanstart?: () => void;
    onscanend?: () => void;
  } = $props();

  let mode: "choose" | "manual" | "uri" = $state("choose");
  let error = $state("");
  let loading = $state(false);
  let scanning = $state(false);
  let scanHint = $state(false);
  let scanHintTimer: ReturnType<typeof setTimeout> | null = null;
  let showWebScanner = $state(false);

  // Manual entry fields
  let issuer = $state("");
  let label = $state("");
  let secret = $state("");
  // URI entry
  let uri = $state("");

  async function ensureCameraPermission(): Promise<boolean> {
    try {
      let state = await checkPermissions();
      if (state === "granted") return true;

      state = await requestPermissions();
      if (state === "granted") return true;

      // Denied — offer to open settings
      error = $_('scanner.cameraPermissionDenied');
      permissionDenied = true;
      return false;
    } catch {
      // Plugin not available (desktop)
      return true;
    }
  }

  let permissionDenied = $state(false);
  let scanCancelReject: ((reason: Error) => void) | null = null;

  async function handleOpenSettings() {
    try {
      await openAppSettings();
    } catch {
      // Silent fail
    }
  }

  async function scanQr() {
    error = "";
    permissionDenied = false;
    let scanStarted = false;
    try {
      const granted = await ensureCameraPermission();
      if (!granted) return;

      scanning = true;
      scanHint = false;
      scanStarted = true;
      document.documentElement.classList.add('scanning');
      onscanstart?.();
      scanHintTimer = setTimeout(() => { scanHint = true; }, 8000);

      // Race scan() against a manual cancel signal — the plugin's cancel()
      // has a bug where it nulls savedInvoke before rejecting, so scan()
      // is never properly rejected on cancel
      const scanPromise = scan({ windowed: true, formats: [Format.QRCode] });
      const cancelPromise = new Promise<never>((_, reject) => {
        scanCancelReject = reject;
      });
      const result = await Promise.race([scanPromise, cancelPromise]);

      if (!result.content) {
        error = $_('scanner.noQrDetected');
        return;
      }

      if (result.content.startsWith("otpauth-migration://")) {
        // Google Authenticator bulk export — hand off to import flow
        const data = Array.from(new TextEncoder().encode(result.content));
        onmigration(data);
        return;
      }

      if (!result.content.startsWith("otpauth://")) {
        error = $_('addAccount.invalidQr');
        return;
      }

      // Add account via the scanned URI
      await addAccount(result.content);
      onsuccess();
    } catch (e: unknown) {
      const raw = e instanceof Error ? e.message : typeof e === "string" ? e : String(e);
      const msg = raw.toLowerCase();
      if (msg.includes("cancel")) {
        // User cancelled — not an error
      } else if (msg.includes("permission") || msg.includes("camera")) {
        error = $_('scanner.cameraPermissionDenied');
        permissionDenied = true;
      } else if (msg.includes("not supported") || msg.includes("not available") || msg.includes("not implemented") || msg.includes("not found")) {
        showWebScanner = true;
      } else {
        error = raw;
      }
    } finally {
      scanCancelReject = null;
      if (scanHintTimer) { clearTimeout(scanHintTimer); scanHintTimer = null; }
      scanHint = false;
      if (scanStarted) {
        scanning = false;
        document.documentElement.classList.remove('scanning');
        onscanend?.();
      }
    }
  }

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
      await addAccount(uri.trim());
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function handleWebScanResult(content: string) {
    showWebScanner = false;
    error = "";

    if (content.startsWith("otpauth-migration://")) {
      const data = Array.from(new TextEncoder().encode(content));
      onmigration(data);
      return;
    }

    if (!content.startsWith("otpauth://")) {
      error = $_('addAccount.invalidQr');
      return;
    }

    try {
      await addAccount(content);
      onsuccess();
    } catch (e) {
      error = String(e);
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

  async function cancelScan() {
    // Call native cancel first — cleans up camera and restores webview state
    try { await cancel(); } catch { /* silent */ }
    // Then reject the scan promise (still needed for Android where
    // cancel() doesn't properly reject the scan promise)
    scanCancelReject?.(new Error("cancelled"));
    scanCancelReject = null;
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
      {#if permissionDenied}
        <button
          type="button"
          class="w-full text-left border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-sm hover:border-error/50 transition-colors"
          onclick={handleOpenSettings}
        >
          <span class="text-error/60">{$_('common.errorPrefix')}</span> {error}
        </button>
      {:else}
        <div class="border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-sm">
          <span class="text-error/60">{$_('common.errorPrefix')}</span> {error}
        </div>
      {/if}
    {/if}

    {#if mode === "choose"}
      <div class="flex flex-col gap-2">
        <button
          type="button"
          class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
          disabled={scanning}
          onclick={scanQr}
        >
          <div class="text-base text-fg group-hover:text-fg flex items-center gap-2">
            <img src={iconQr} alt="" class="w-4 h-4 icon-adapt opacity-60" />
            {scanning ? $_('scanner.scanning') : $_('scanner.scanQrCode')}
          </div>
          <div class="text-sm text-dim mt-1 ml-6">{$_('addAccount.scanQrDesc')}</div>
        </button>
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
        <button
          type="button"
          class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
          onclick={onimportexternal}
        >
          <div class="text-base text-fg group-hover:text-fg flex items-center gap-2">
            <img src={iconApp} alt="" class="w-4 h-4 icon-adapt opacity-60" />
            {$_('addAccount.importFromApp')}
          </div>
          <div class="text-sm text-dim mt-1 ml-6">{$_('addAccount.importFromAppDesc')}</div>
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

{#if showWebScanner}
  <WebScanner
    onscan={handleWebScanResult}
    oncancel={() => { showWebScanner = false; }}
  />
{/if}

{#if scanning}
  <div class="fixed inset-0 z-[100] pointer-events-none scan-overlay">
    <button
      type="button"
      class="pointer-events-auto absolute right-5 w-10 h-10 flex items-center justify-center bg-bg/50 backdrop-blur-sm rounded-full"
      style="top: calc(env(safe-area-inset-top, 56px) + 0.75rem)"
      onclick={cancelScan}
      aria-label={$_('scanner.closeCamera')}
    >
      <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" class="text-fg">
        <line x1="2" y1="2" x2="12" y2="12" /><line x1="12" y1="2" x2="2" y2="12" />
      </svg>
    </button>
    <div class="absolute inset-x-12 top-1/2 -translate-y-1/2 aspect-square">
      <div class="absolute top-0 left-0 w-8 h-8 border-t-[3px] border-l-[3px] border-fg/80"></div>
      <div class="absolute top-0 right-0 w-8 h-8 border-t-[3px] border-r-[3px] border-fg/80"></div>
      <div class="absolute bottom-0 left-0 w-8 h-8 border-b-[3px] border-l-[3px] border-fg/80"></div>
      <div class="absolute bottom-0 right-0 w-8 h-8 border-b-[3px] border-r-[3px] border-fg/80"></div>
    </div>
    {#if scanHint}
      <p class="pointer-events-none absolute left-0 right-0 bottom-[20%] text-center text-xs text-fg/70 px-8">
        {$_('scanner.scanHint')}
      </p>
    {/if}
  </div>
{/if}
