<script lang="ts">
  import { storage, loadAccounts } from "$lib/stores/accounts.svelte";
  import { keyFromSyncCode, formatSyncCode, parseSyncUri } from "$core/sync-code";
  import { buildPayload, decryptAccount, merge } from "$core/sync-protocol";
  import { syncViaWebSocketAnyHost } from "$core/sync-ws-client";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { toast } from "$lib/stores/toast";
  import { inputClass, btnPrimary, btnSecondary } from "$lib/styles/styles";
  import type { Account, MergeResult, MergeConflict } from "$core/types";
  import { _ } from 'svelte-i18n';
  import iconPhone from "$lib/assets/icons/iphone.svg";
  import iconArrow from "$lib/assets/icons/right-arrow.svg";

  let { onclose, onsuccess }: { onclose: () => void; onsuccess: () => void } = $props();

  let rawCode = $state("");
  let address = $state("");
  let error = $state("");
  let loading = $state(false);
  let mergeResult: MergeResult | null = $state(null);
  let decisions: Map<string, string> = $state(new Map());

  let mounted = $state(false);

  const CODE_SEGMENTS = 6;
  const SEGMENT_LEN = 4;
  const TOTAL_CHARS = CODE_SEGMENTS * SEGMENT_LEN;

  let code = $derived(formatSyncCode(rawCode));

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

  let scrollLeft = $state(0);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 250);
  }

  function handleCodeInput(e: Event) {
    const input = e.target as HTMLInputElement;
    const raw = input.value.replace(/[^a-zA-Z0-9]/g, "").slice(0, TOTAL_CHARS);
    rawCode = raw;

    requestAnimationFrame(() => {
      const formatted = formatSyncCode(raw);
      input.value = formatted;
      const charCount = raw.length;
      const dashCount = charCount > 0 ? Math.floor((charCount - 1) / SEGMENT_LEN) : 0;
      const pos = charCount + dashCount;
      input.setSelectionRange(pos, pos);
      scrollLeft = input.scrollLeft;
    });
  }

  async function handleConnect() {
    error = "";
    if (!rawCode.trim()) {
      error = $_('ext.sync.syncCodeRequired');
      return;
    }
    if (!address.trim()) {
      error = $_('ext.sync.addressRequired');
      return;
    }

    // Parse address — host:port format
    const parts = address.trim().split(":");
    if (parts.length !== 2) {
      error = $_('ext.sync.addressFormatError');
      return;
    }
    const host = parts[0];
    const port = parseInt(parts[1], 10);
    if (isNaN(port) || port < 1 || port > 65535) {
      error = $_('ext.sync.invalidPort');
      return;
    }

    // Use pasted hosts if available (multi-host), otherwise manual single host
    const hosts = pastedHosts.length > 0 ? pastedHosts : [host];
    await connectToDevice(code.trim(), hosts, port);
  }

  // Hosts parsed from URI (for multi-host connect)
  let pastedHosts: string[] = [];

  async function handlePaste() {
    error = "";
    try {
      const text = await navigator.clipboard.readText();
      if (text.startsWith("ghost-auth://sync")) {
        const parsed = parseSyncUri(text);
        rawCode = parsed.code;
        pastedHosts = parsed.hosts;
        address = `${parsed.hosts[0]}:${parsed.port}`;
        toast($_('ext.sync.uriPasted'));
      } else {
        error = $_('ext.sync.uriInvalid');
      }
    } catch {
      error = $_('ext.sync.clipboardFailed');
    }
  }

  async function connectToDevice(syncCode: string, hosts: string[], wsPort: number) {
    loading = true;
    try {
      const key = await keyFromSyncCode(syncCode);

      // Build local payload
      const accounts = await storage.getAccounts();
      const tombstones = await storage.getTombstones();
      const deviceId = await storage.getDeviceId();
      const localPayload = await buildPayload(deviceId, accounts, tombstones, key);

      // Connect, handshake, exchange payloads (tries all hosts)
      const remotePayload = await syncViaWebSocketAnyHost(hosts, wsPort, key, localPayload);

      // Decrypt remote accounts
      const remoteAccounts: Account[] = [];
      for (const enc of remotePayload.accounts) {
        remoteAccounts.push(await decryptAccount(enc, key));
      }

      // Load sync history for this peer
      const syncHistory = await storage.getSyncHistory();
      const lastSync = syncHistory[remotePayload.device_id] ?? null;

      // Compute merge
      mergeResult = merge(
        accounts,
        tombstones,
        remoteAccounts,
        remotePayload.tombstones,
        lastSync,
      );

      // Store remote info for applying later
      (mergeResult as any)._remoteDeviceId = remotePayload.device_id;
      (mergeResult as any)._remoteTimestamp = remotePayload.timestamp;

      // Auto-confirm if no conflicts or deletions
      if (mergeResult.conflicts.length === 0 && mergeResult.remote_deletions.length === 0) {
        await applyMerge();
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function applyMerge() {
    if (!mergeResult) return;
    loading = true;
    try {
      const accounts = await storage.getAccounts();
      const tombstones = await storage.getTombstones();
      const accountMap = new Map(accounts.map((a) => [a.id, a]));

      // Apply additions
      for (const account of mergeResult.to_add) {
        if (!accountMap.has(account.id)) {
          accounts.push(account);
          accountMap.set(account.id, account);
        }
      }

      // Apply auto-updates
      for (const updated of mergeResult.auto_updated) {
        const idx = accounts.findIndex((a) => a.id === updated.id);
        if (idx !== -1) {
          accounts[idx] = updated;
        }
      }

      // Apply conflict decisions
      for (const conflict of mergeResult.conflicts) {
        const decision = decisions.get(conflict.local.id) || "keep_local";
        if (decision === "keep_remote") {
          const idx = accounts.findIndex((a) => a.id === conflict.remote.id);
          if (idx !== -1) {
            accounts[idx] = conflict.remote;
          }
        }
        // keep_local = do nothing
      }

      // Apply remote deletions (only if user chose to delete)
      const newTombstones = [...tombstones];
      for (const del of mergeResult.remote_deletions) {
        const decision = decisions.get(del.id) || "keep_local";
        if (decision === "delete") {
          const idx = accounts.findIndex((a) => a.id === del.id);
          if (idx !== -1) {
            accounts.splice(idx, 1);
            newTombstones.push({ id: del.id, deleted_at: Math.floor(Date.now() / 1000) });
          }
        }
      }

      await storage.saveAccounts(accounts, newTombstones);

      // Record sync timestamp
      const remoteDeviceId = (mergeResult as any)._remoteDeviceId;
      const remoteTimestamp = (mergeResult as any)._remoteTimestamp;
      if (remoteDeviceId) {
        const syncHistory = await storage.getSyncHistory();
        syncHistory[remoteDeviceId] = remoteTimestamp || Math.floor(Date.now() / 1000);
        await storage.saveSyncHistory(syncHistory);
      }

      await loadAccounts();

      const parts = [];
      if (mergeResult.to_add.length > 0) parts.push($_('sync.toastAdded', { values: { count: mergeResult.to_add.length } }));
      if (mergeResult.auto_updated.length > 0) parts.push($_('sync.toastUpdated', { values: { count: mergeResult.auto_updated.length } }));
      toast($_('sync.toastSynced', { values: { summary: parts.join(", ") || $_('sync.toastNoChanges') } }));
      onsuccess();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function setDecision(id: string, action: string) {
    decisions = new Map(decisions);
    decisions.set(id, action);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-50 flex items-end justify-center modal-backdrop {mounted ? 'open' : ''}"
  onkeydown={(e) => e.key === "Escape" && close()}
  onclick={close}
  role="presentation"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="bg-bg border-t border-dotted border-border w-full p-5 max-h-[85vh] overflow-y-auto modal-panel {mounted ? 'open' : ''}"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="sync-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-6">
      <span id="sync-title" class="text-base tracking-wide text-muted">{$_('ext.sync.title')}</span>
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
      <div class="border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-sm">
        <span class="text-error/60">{$_('common.errorPrefix')}</span> {error}
      </div>
    {/if}

    {#if !mergeResult}
      <div class="flex items-center gap-2 text-sm text-dim mb-4">
        <span class="flex items-center gap-1">
          <img src={iconArrow} alt="" class="w-2.5 h-2.5 icon-adapt opacity-35" style="transform: scaleX(-1)" />
          <img src={iconPhone} alt="" class="w-3.5 h-3.5 icon-adapt opacity-50" />
        </span>
        <p>{$_('ext.sync.instructions')}</p>
      </div>

      <form
        class="flex flex-col gap-3"
        onsubmit={(e) => { e.preventDefault(); handleConnect(); }}
      >
        <div>
          <label for="sync-code" class="block text-sm text-dim tracking-wide mb-1.5">{$_('ext.sync.syncCodeLabel')}</label>
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
          <label for="sync-address" class="block text-sm text-dim tracking-wide mb-1.5">{$_('ext.sync.addressLabel')}</label>
          <input
            id="sync-address"
            type="text"
            bind:value={address}
            placeholder={$_('ext.sync.addressPlaceholder')}
            class={inputClass}
          />
        </div>

        <div class="flex gap-2 mt-3">
          <button type="button" class={btnSecondary} onclick={close}>
            {$_('common.cancel')}
          </button>
          <button type="button" class={btnSecondary} onclick={handlePaste}>
            {$_('ext.sync.pasteUri')}
          </button>
          <button
            type="submit"
            disabled={loading}
            class="{btnPrimary} disabled:opacity-30"
          >
            {loading ? $_('ext.sync.connecting') : $_('ext.sync.connect')}
          </button>
        </div>
      </form>
    {:else}
      <!-- Merge Preview -->
      <div class="flex flex-col gap-4">
        {#if mergeResult.to_add.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.newAccounts', { values: { count: mergeResult.to_add.length } })}
            </p>
            <div class="flex flex-col gap-1">
              {#each mergeResult.to_add as account}
                <div class="border border-dotted border-border px-4 py-2.5">
                  <div class="text-sm text-fg">{account.issuer}</div>
                  <div class="text-xs text-dim">{account.label}</div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mergeResult.auto_updated.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.autoUpdated', { values: { count: mergeResult.auto_updated.length } })}
            </p>
            <div class="flex flex-col gap-1">
              {#each mergeResult.auto_updated as account}
                <div class="border border-dotted border-border px-4 py-2.5">
                  <div class="text-sm text-fg">{account.issuer}</div>
                  <div class="text-xs text-dim">{account.label}</div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mergeResult.conflicts.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.conflicts', { values: { count: mergeResult.conflicts.length } })}
            </p>
            <div class="flex flex-col gap-2">
              {#each mergeResult.conflicts as conflict}
                <div class="border border-dotted border-border px-4 py-3">
                  <div class="flex gap-3 mb-2">
                    <div class="flex-1">
                      <div class="text-xs text-dim mb-1">{$_('sync.thisDevice')}</div>
                      <div class="text-sm text-fg">{conflict.local.issuer}</div>
                      <div class="text-xs text-dim">{conflict.local.label}</div>
                    </div>
                    <div class="flex-1">
                      <div class="text-xs text-dim mb-1">{$_('sync.otherDevice')}</div>
                      <div class="text-sm text-fg">{conflict.remote.issuer}</div>
                      <div class="text-xs text-dim">{conflict.remote.label}</div>
                    </div>
                  </div>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(conflict.local.id) === 'keep_local' ? 'border-fg/80 text-fg' : 'border-dotted border-border text-dim hover:text-fg'}"
                      onclick={() => setDecision(conflict.local.id, "keep_local")}
                    >
                      {$_('sync.keepThis')}
                    </button>
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(conflict.local.id) === 'keep_remote' ? 'border-fg/80 text-fg' : 'border-dotted border-border text-dim hover:text-fg'}"
                      onclick={() => setDecision(conflict.local.id, "keep_remote")}
                    >
                      {$_('sync.keepOther')}
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if mergeResult.remote_deletions.length > 0}
          <div>
            <p class="text-sm text-dim tracking-wide mb-2">
              {$_('sync.deletedOnOther', { values: { count: mergeResult.remote_deletions.length } })}
            </p>
            <div class="flex flex-col gap-2">
              {#each mergeResult.remote_deletions as account}
                <div class="border border-dotted border-border px-4 py-3">
                  <div class="text-sm text-fg mb-2">
                    {account.issuer}
                    <span class="text-dim">/ {account.label}</span>
                  </div>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(account.id) !== 'delete' ? 'border-fg/80 text-fg' : 'border-dotted border-border text-dim hover:text-fg'}"
                      onclick={() => setDecision(account.id, "keep_local")}
                    >
                      {$_('sync.keep')}
                    </button>
                    <button
                      type="button"
                      class="flex-1 text-xs py-1.5 border transition-colors {decisions.get(account.id) === 'delete' ? 'border-error/80 text-error' : 'border-dotted border-border text-dim hover:text-error'}"
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

        {#if mergeResult.unchanged > 0}
          <p class="text-sm text-dim">
            {$_('sync.unchanged', { values: { count: mergeResult.unchanged } })}
          </p>
        {/if}

        <div class="flex gap-2 mt-2">
          <button type="button" class={btnSecondary} onclick={close}>
            {$_('common.cancel')}
          </button>
          <button
            type="button"
            disabled={loading}
            class="{btnPrimary} disabled:opacity-30"
            onclick={applyMerge}
          >
            {loading ? $_('common.loading') : $_('sync.applySync')}
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>
