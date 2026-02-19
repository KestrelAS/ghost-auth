<script lang="ts">
  import { _ } from 'svelte-i18n';
  import {
    syncJoin,
    syncConfirm,
    syncCancel,
    type MergePreview,
    type MergeDecision,
  } from "$lib/stores/accounts";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { toast } from "$lib/stores/toast";
  import { inputClass, btnPrimary, btnSecondary } from "$lib/styles";
  import { scan, cancel, Format, checkPermissions, requestPermissions, openAppSettings } from "@tauri-apps/plugin-barcode-scanner";
  import WebScanner from "./WebScanner.svelte";
  import iconQr from "$lib/assets/icons/qr.svg";
  import iconManualEntry from "$lib/assets/icons/manual-entry.svg";

  let { onclose, onsuccess, onscanstart, onscanend }: {
    onclose: () => void;
    onsuccess: () => void;
    onscanstart?: () => void;
    onscanend?: () => void;
  } = $props();

  let mode: "choose" | "manual" = $state("choose");
  let rawCode = $state("");
  let address = $state("");

  const CODE_SEGMENTS = 6;
  const SEGMENT_LEN = 4;
  const TOTAL_CHARS = CODE_SEGMENTS * SEGMENT_LEN; // 24

  let code = $derived(formatCodeWithDashes(rawCode));
  let scrollLeft = $state(0);

  function formatCodeWithDashes(raw: string): string {
    const clean = raw.replace(/[^a-zA-Z0-9]/g, "").slice(0, TOTAL_CHARS);
    const segments: string[] = [];
    for (let i = 0; i < clean.length; i += SEGMENT_LEN) {
      segments.push(clean.slice(i, i + SEGMENT_LEN));
    }
    return segments.join("-");
  }

  function handleCodeInput(e: Event) {
    const input = e.target as HTMLInputElement;
    const raw = input.value.replace(/[^a-zA-Z0-9]/g, "").slice(0, TOTAL_CHARS);
    rawCode = raw;

    // Restore cursor position after formatting
    requestAnimationFrame(() => {
      const formatted = formatCodeWithDashes(raw);
      input.value = formatted;
      // Place cursor at end of actual input
      const charCount = raw.length;
      const dashCount = charCount > 0 ? Math.floor((charCount - 1) / SEGMENT_LEN) : 0;
      const pos = charCount + dashCount;
      input.setSelectionRange(pos, pos);
      scrollLeft = input.scrollLeft;
    });
  }

  let codeDisplay = $derived.by(() => {
    const clean = rawCode.replace(/[^a-zA-Z0-9]/g, "").slice(0, TOTAL_CHARS).toUpperCase();
    const segments: Array<{ typed: string; placeholder: string }> = [];
    for (let i = 0; i < CODE_SEGMENTS; i++) {
      const start = i * SEGMENT_LEN;
      const chunk = clean.slice(start, start + SEGMENT_LEN);
      segments.push({
        typed: chunk,
        placeholder: "X".repeat(SEGMENT_LEN - chunk.length),
      });
    }
    return segments;
  });
  let error = $state("");
  let loading = $state(false);
  let scanning = $state(false);
  let scanHint = $state(false);
  let scanHintTimer: ReturnType<typeof setTimeout> | null = null;
  let showWebScanner = $state(false);
  let permissionDenied = $state(false);
  let scanCancelReject: ((reason: Error) => void) | null = null;
  let mergePreview: MergePreview | null = $state(null);
  let decisions: Map<string, string> = $state(new Map());

  let mounted = $state(false);

  $effect(() => {
    requestAnimationFrame(() => {
      mounted = true;
    });
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

      if (!result.content.startsWith("ghost-auth://sync")) {
        error = $_('syncJoin.invalidSyncQr');
        return;
      }

      const url = new URL(result.content);
      const scannedCode = url.searchParams.get("code");
      const scannedPort = url.searchParams.get("port");

      // Parse hosts: prefer comma-separated "hosts" param, fall back to single "host"
      const hostsParam = url.searchParams.get("hosts");
      const hostParam = url.searchParams.get("host");
      const scannedHosts = hostsParam
        ? hostsParam.split(",").filter(Boolean)
        : hostParam ? [hostParam] : [];

      if (!scannedCode || scannedHosts.length === 0 || !scannedPort) {
        error = $_('syncJoin.missingSyncData');
        return;
      }

      const port = parseInt(scannedPort, 10);
      if (isNaN(port) || port < 1 || port > 65535) {
        error = $_('syncJoin.badPort');
        return;
      }

      await connectToDeviceMulti(scannedCode, scannedHosts, port);
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

    if (!content.startsWith("ghost-auth://sync")) {
      error = $_('syncJoin.invalidSyncQr');
      return;
    }

    try {
      const url = new URL(content);
      const scannedCode = url.searchParams.get("code");
      const scannedPort = url.searchParams.get("port");
      const hostsParam = url.searchParams.get("hosts");
      const hostParam = url.searchParams.get("host");
      const scannedHosts = hostsParam
        ? hostsParam.split(",").filter(Boolean)
        : hostParam ? [hostParam] : [];

      if (!scannedCode || scannedHosts.length === 0 || !scannedPort) {
        error = $_('syncJoin.missingSyncData');
        return;
      }

      const port = parseInt(scannedPort, 10);
      if (isNaN(port) || port < 1 || port > 65535) {
        error = $_('syncJoin.badPort');
        return;
      }

      await connectToDeviceMulti(scannedCode, scannedHosts, port);
    } catch (e) {
      error = String(e);
    }
  }

  async function connectToDeviceMulti(syncCode: string, hosts: string[], port: number) {
    loading = true;
    let lastError = "";
    for (const host of hosts) {
      try {
        mergePreview = await syncJoin(syncCode, host, port);
        if (
          mergePreview.conflicts.length === 0 &&
          mergePreview.to_delete.length === 0
        ) {
          await handleAutoConfirm();
        }
        loading = false;
        return;
      } catch (e) {
        lastError = String(e);
      }
    }
    error = lastError || $_('syncJoin.connectionFailed');
    loading = false;
  }

  async function connectToDevice(syncCode: string, host: string, port: number) {
    await connectToDeviceMulti(syncCode, [host], port);
  }

  async function handleConnect() {
    error = "";
    if (!rawCode.trim()) {
      error = $_('syncJoin.syncCodeRequired');
      return;
    }
    if (!address.trim()) {
      error = $_('syncJoin.addressRequired');
      return;
    }

    const parts = address.trim().split(":");
    if (parts.length !== 2) {
      error = $_('syncJoin.addressFormat');
      return;
    }
    const host = parts[0];
    const port = parseInt(parts[1], 10);
    if (isNaN(port) || port < 1 || port > 65535) {
      error = $_('syncJoin.invalidPort');
      return;
    }

    await connectToDevice(code.trim(), host, port);
  }

  async function handleAutoConfirm() {
    loading = true;
    try {
      const result = await syncConfirm([]);
      const parts = [];
      if (result.added > 0) parts.push($_('sync.toastAdded', { values: { count: result.added } }));
      if (result.updated > 0) parts.push($_('sync.toastUpdated', { values: { count: result.updated } }));
      toast($_('sync.toastSynced', { values: { summary: parts.join(", ") || $_('sync.toastNoChanges') } }));
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function handleConfirm() {
    loading = true;
    try {
      const decs: MergeDecision[] = [];
      for (const [id, action] of decisions) {
        decs.push({ account_id: id, action });
      }
      const result = await syncConfirm(decs);
      const parts = [];
      if (result.added > 0) parts.push($_('sync.toastAdded', { values: { count: result.added } }));
      if (result.updated > 0) parts.push($_('sync.toastUpdated', { values: { count: result.updated } }));
      if (result.deleted > 0) parts.push($_('sync.toastDeleted', { values: { count: result.deleted } }));
      toast($_('sync.toastSynced', { values: { summary: parts.join(", ") || $_('sync.toastNoChanges') } }));
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function handleCancel() {
    try {
      await syncCancel();
    } catch {
      // ignore
    }
    close();
  }

  function setDecision(id: string, action: string) {
    decisions = new Map(decisions);
    decisions.set(id, action);
  }

  async function cancelScan() {
    try { await cancel(); } catch { /* silent */ }
    scanCancelReject?.(new Error("cancelled"));
    scanCancelReject = null;
  }

</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-50 flex items-end sm:items-center justify-center modal-backdrop {mounted
    ? 'open'
    : ''}"
  onkeydown={(e) => e.key === "Escape" && handleCancel()}
  onclick={handleCancel}
  role="presentation"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="bg-bg border-t border-dotted border-border sm:border w-full max-w-md p-5 max-h-[85vh] overflow-y-auto modal-panel {mounted
      ? 'open'
      : ''}"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="sync-join-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-6">
      <span id="sync-join-title" class="text-base tracking-wide text-muted"
        >{$_('syncJoin.title')}</span
      >
      <button
        type="button"
        class="text-dim hover:text-fg transition-colors p-1"
        onclick={handleCancel}
        aria-label={$_('common.close')}
      >
        <svg
          width="18"
          height="18"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
        >
          <line x1="2" y1="2" x2="12" y2="12" /><line
            x1="12"
            y1="2"
            x2="2"
            y2="12"
          />
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
        <div
          class="border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-sm"
        >
          <span class="text-error/60">{$_('common.errorPrefix')}</span>
          {error}
        </div>
      {/if}
    {/if}

    {#if !mergePreview && mode === "choose"}
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
          <div class="text-sm text-dim mt-1 ml-6">{$_('syncJoin.scanQrDesc')}</div>
        </button>
        <button
          type="button"
          class="text-left border border-dotted border-border px-4 py-3 hover:border-fg/30 transition-colors group"
          onclick={() => (mode = "manual")}
        >
          <div class="text-base text-fg group-hover:text-fg flex items-center gap-2">
            <img src={iconManualEntry} alt="" class="w-4 h-4 icon-adapt opacity-60" />
            {$_('syncJoin.enterManually')}
          </div>
          <div class="text-sm text-dim mt-1 ml-6">{$_('syncJoin.enterManuallyDesc')}</div>
        </button>
      </div>
    {:else if !mergePreview && mode === "manual"}
      <form
        class="flex flex-col gap-3"
        onsubmit={(e) => {
          e.preventDefault();
          handleConnect();
        }}
      >
        <div>
          <label
            for="sync-code"
            class="block text-sm text-dim tracking-wide mb-1.5"
            >{$_('syncJoin.syncCodeLabel')}</label
          >
          <div class="relative overflow-hidden">
            <input
              id="sync-code"
              type="text"
              value={code}
              oninput={handleCodeInput}
              onscroll={(e) => { scrollLeft = (e.target as HTMLInputElement).scrollLeft; }}
              maxlength={29}
              autocomplete="off"
              autocapitalize="characters"
              spellcheck={false}
              class="{inputClass} uppercase tracking-[0.18em] font-mono !text-transparent !caret-fg"
            />
            <div
              class="absolute inset-0 pointer-events-none flex items-center px-3 tracking-[0.18em] font-mono text-base"
              aria-hidden="true"
            >
              <span class="flex items-center" style="transform: translateX(-{scrollLeft}px)">
                {#each codeDisplay as seg, i}
                  {#if i > 0}<span class="text-dim/30">-</span>{/if}
                  <span class="text-fg uppercase">{seg.typed}</span>
                  <span class="text-dim/30">{seg.placeholder}</span>
                {/each}
              </span>
            </div>
          </div>
        </div>
        <div>
          <label
            for="sync-address"
            class="block text-sm text-dim tracking-wide mb-1.5"
            >{$_('syncJoin.addressLabel')}</label
          >
          <input
            id="sync-address"
            type="text"
            bind:value={address}
            placeholder={$_('syncJoin.addressPlaceholder')}
            class={inputClass}
          />
        </div>

        <div class="flex gap-2 mt-3">
          <button type="button" class={btnSecondary} onclick={() => (mode = "choose")}>
            {$_('common.back')}
          </button>
          <button
            type="submit"
            disabled={loading}
            class="{btnPrimary} disabled:opacity-30"
          >
            {loading ? $_('syncJoin.connecting') : $_('syncJoin.connect')}
          </button>
        </div>
      </form>
    {:else if mergePreview}
      <!-- Merge Preview -->
      <div class="flex flex-col gap-4">
        {#if mergePreview.to_add.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.newAccounts', { values: { count: mergePreview.to_add.length } })}
            </p>
            <div class="flex flex-col gap-1">
              {#each mergePreview.to_add as account}
                <div class="border border-dotted border-border px-4 py-2.5">
                  <div class="text-sm text-fg">{account.issuer}</div>
                  <div class="text-xs text-dim">{account.label}</div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mergePreview.auto_updated.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.autoUpdated', { values: { count: mergePreview.auto_updated.length } })}
            </p>
            <div class="flex flex-col gap-1">
              {#each mergePreview.auto_updated as account}
                <div class="border border-dotted border-border px-4 py-2.5">
                  <div class="text-sm text-fg">{account.issuer}</div>
                  <div class="text-xs text-dim">{account.label}</div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mergePreview.conflicts.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.conflicts', { values: { count: mergePreview.conflicts.length } })}
            </p>
            <div class="flex flex-col gap-2">
              {#each mergePreview.conflicts as conflict}
                <div class="border border-dotted border-border px-4 py-3">
                  <div class="flex gap-3 mb-2">
                    <div class="flex-1">
                      <div class="text-xs text-dim mb-1">
                        {$_('sync.thisDevice')}
                      </div>
                      <div class="text-sm text-fg">
                        {conflict.local.issuer}
                      </div>
                      <div class="text-xs text-dim">
                        {conflict.local.label}
                      </div>
                    </div>
                    <div class="flex-1">
                      <div class="text-xs text-dim mb-1">
                        {$_('sync.otherDevice')}
                      </div>
                      <div class="text-sm text-fg">
                        {conflict.remote.issuer}
                      </div>
                      <div class="text-xs text-dim">
                        {conflict.remote.label}
                      </div>
                    </div>
                  </div>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(
                        conflict.account_id,
                      ) === 'keep_local'
                        ? 'border-fg/80 text-fg'
                        : 'border-dotted border-border text-dim hover:text-fg'}"
                      onclick={() =>
                        setDecision(conflict.account_id, "keep_local")}
                    >
                      {$_('sync.keepThis')}
                    </button>
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(
                        conflict.account_id,
                      ) === 'keep_remote'
                        ? 'border-fg/80 text-fg'
                        : 'border-dotted border-border text-dim hover:text-fg'}"
                      onclick={() =>
                        setDecision(conflict.account_id, "keep_remote")}
                    >
                      {$_('sync.keepOther')}
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mergePreview.to_delete.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.deletedOnOther', { values: { count: mergePreview.to_delete.length } })}
            </p>
            <div class="flex flex-col gap-2">
              {#each mergePreview.to_delete as account}
                <div class="border border-dotted border-border px-4 py-3">
                  <div class="text-sm text-fg mb-2">
                    {account.issuer}
                    <span class="text-dim">/ {account.label}</span>
                  </div>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(
                        account.id,
                      ) !== 'delete'
                        ? 'border-fg/80 text-fg'
                        : 'border-dotted border-border text-dim hover:text-fg'}"
                      onclick={() => setDecision(account.id, "keep_local")}
                    >
                      {$_('sync.keep')}
                    </button>
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(
                        account.id,
                      ) === 'delete'
                        ? 'border-error/80 text-error'
                        : 'border-dotted border-border text-dim hover:text-error'}"
                      onclick={() => setDecision(account.id, "delete")}
                    >
                      {$_('common.delete')}
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mergePreview.unchanged > 0}
          <p class="text-sm text-dim">
            {$_('sync.unchanged', { values: { count: mergePreview.unchanged } })}
          </p>
        {/if}

        <div class="flex gap-2 mt-2">
          <button
            type="button"
            class={btnSecondary}
            onclick={handleCancel}
          >
            {$_('common.cancel')}
          </button>
          <button
            type="button"
            disabled={loading}
            class="{btnPrimary} disabled:opacity-30"
            onclick={handleConfirm}
          >
            {loading ? $_('common.loading') : $_('sync.applySync')}
          </button>
        </div>
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
