<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { importExternalPreview, importExternalConfirm, type ImportPreview } from "$lib/stores/accounts";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { toast } from "$lib/stores/toast";
  import { btnPrimary, btnSecondary } from "$lib/styles";
  import { scan, cancel, Format, checkPermissions, requestPermissions, openAppSettings } from "@tauri-apps/plugin-barcode-scanner";
  import WebScanner from "./WebScanner.svelte";
  import iconQr from "$lib/assets/icons/qr.svg";
  import iconImportFile from "$lib/assets/icons/import-file.svg";

  let { onclose, onsuccess, initialData, onscanstart, onscanend }: {
    onclose: () => void;
    onsuccess: () => void;
    initialData?: number[];
    onscanstart?: () => void;
    onscanend?: () => void;
  } = $props();

  let mode: "choose" | "file" = $state(initialData ? "file" : "choose");
  let fileData: number[] | null = $state(initialData ?? null);
  let fileName = $state("");
  let error = $state("");
  let loading = $state(false);
  let scanning = $state(false);
  let scanHint = $state(false);
  let scanHintTimer: ReturnType<typeof setTimeout> | null = null;
  let showWebScanner = $state(false);
  let permissionDenied = $state(false);
  let scanCancelReject: ((reason: Error) => void) | null = null;
  let preview: ImportPreview | null = $state(null);

  let mounted = $state(false);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  // Auto-preview when opened with pre-loaded data (e.g. from QR scan)
  $effect(() => {
    if (initialData && !preview && !loading && !error) {
      handlePreview();
    }
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
  }

  async function ensureCameraPermission(): Promise<boolean> {
    try {
      let state = await checkPermissions();
      if (state === "granted") return true;
      state = await requestPermissions();
      if (state === "granted") return true;
      error = $_('scanner.cameraPermissionDenied');
      permissionDenied = true;
      return false;
    } catch {
      return true;
    }
  }

  async function handleOpenSettings() {
    try {
      await openAppSettings();
    } catch {
      // silent
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

      const scanPromise = scan({ windowed: true, formats: [Format.QRCode] });
      const cancelPromise = new Promise<never>((_, reject) => {
        scanCancelReject = reject;
      });
      const result = await Promise.race([scanPromise, cancelPromise]);
      if (!result.content) {
        error = $_('scanner.noQrDetected');
        return;
      }

      if (!result.content.startsWith("otpauth-migration://") && !result.content.startsWith("otpauth://")) {
        error = $_('importExternal.invalidQr');
        return;
      }

      fileData = Array.from(new TextEncoder().encode(result.content));
      mode = "file";
      await handlePreview();
    } catch (e: unknown) {
      const raw = e instanceof Error ? e.message : typeof e === "string" ? e : String(e);
      const msg = raw.toLowerCase();
      if (msg.includes("cancel")) {
        // user cancelled scan
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

  async function handleWebScanResult(content: string) {
    showWebScanner = false;
    error = "";

    if (!content.startsWith("otpauth-migration://") && !content.startsWith("otpauth://")) {
      error = $_('importExternal.invalidQr');
      return;
    }

    fileData = Array.from(new TextEncoder().encode(content));
    mode = "file";
    await handlePreview();
  }

  function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    fileName = file.name;
    const reader = new FileReader();
    reader.onload = () => {
      fileData = Array.from(new Uint8Array(reader.result as ArrayBuffer));
    };
    reader.readAsArrayBuffer(file);
  }

  async function handlePreview() {
    if (!fileData) return;
    error = "";
    loading = true;
    try {
      preview = await importExternalPreview(fileData);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function handleConfirm() {
    if (!fileData) return;
    loading = true;
    error = "";
    try {
      const added = await importExternalConfirm(fileData);
      toast($_('importExternal.imported', { values: { count: added.length, format: preview?.format ?? 'file' } }));
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function cancelScan() {
    try { await cancel(); } catch { /* silent */ }
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
    aria-labelledby="import-external-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-6">
      <span id="import-external-title" class="text-base tracking-wide text-muted">{$_('importExternal.title')}</span>
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

    {#if !preview && mode === "choose"}
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
          <div class="text-sm text-dim mt-1 ml-6">{$_('importExternal.scanQrDesc')}</div>
        </button>
        <button
          type="button"
          class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
          onclick={() => (mode = "file")}
        >
          <div class="text-base text-fg group-hover:text-fg flex items-center gap-2">
            <img src={iconImportFile} alt="" class="w-4 h-4 icon-adapt opacity-60" />
            {$_('importExternal.importFile')}
          </div>
          <div class="text-sm text-dim mt-1 ml-6">{$_('importExternal.importFileDesc')}</div>
        </button>
      </div>
    {:else if !preview && mode === "file"}
      <form
        class="flex flex-col gap-3"
        onsubmit={(e) => { e.preventDefault(); handlePreview(); }}
      >
        <p class="text-sm text-muted mb-1">
          {$_('importExternal.supportedFormats')}
        </p>
        <div>
          <label for="import-file" class="block text-sm text-dim tracking-wide mb-1.5">{$_('importExternal.exportFileLabel')}</label>
          <label class="block border border-dotted border-border px-3 py-2.5 text-base text-dim hover:border-fg/30 transition-colors cursor-pointer">
            {fileName || $_('importExternal.chooseFile')}
            <input
              id="import-file"
              type="file"
              accept=".json,.txt,.2fas"
              class="hidden"
              onchange={handleFileSelect}
            />
          </label>
        </div>

        <div class="flex gap-2 mt-3">
          <button type="button" class={btnSecondary} onclick={() => (mode = "choose")}>
            {$_('common.back')}
          </button>
          <button type="submit" disabled={loading || !fileData} class="{btnPrimary} disabled:opacity-30">
            {loading ? $_('common.loading') : $_('importExternal.scan')}
          </button>
        </div>
      </form>
    {:else if preview}
      <div class="mb-4">
        <p class="text-sm text-muted mb-1">
          {$_('importExternal.detected', { values: { format: preview.format.toLowerCase() } })}
        </p>
        <p class="text-sm text-muted mb-3">
          {$_('importExternal.accountsFound', { values: { total: preview.accounts.length + preview.duplicates } })}{#if preview.duplicates > 0}{$_('importExternal.duplicatesExist', { values: { count: preview.duplicates } })}{/if}{#if preview.skipped > 0}{$_('importExternal.nonTotpSkipped', { values: { count: preview.skipped } })}{/if}.
        </p>
        {#if preview.accounts.length === 0}
          <p class="text-sm text-dim">{$_('importExternal.allExist')}</p>
        {:else}
          <div class="flex flex-col gap-1 max-h-48 overflow-y-auto">
            {#each preview.accounts as account}
              <div class="border border-dotted border-border px-4 py-2.5">
                <div class="text-sm text-fg">{account.issuer}</div>
                <div class="text-xs text-dim">{account.label}</div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex gap-2">
        <button type="button" class={btnSecondary} onclick={() => { preview = null; error = ""; }}>
          {$_('common.back')}
        </button>
        <button type="button" disabled={loading || preview.accounts.length === 0} class="{btnPrimary} disabled:opacity-30" onclick={handleConfirm}>
          {loading ? $_('common.loading') : $_('common.import')}
        </button>
      </div>
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
