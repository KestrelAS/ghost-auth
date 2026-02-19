<script lang="ts">
  import { _ } from 'svelte-i18n';
  import {
    syncStart,
    syncPoll,
    syncConfirm,
    syncCancel,
    type SyncSessionInfo,
    type MergePreview,
    type MergeDecision,
  } from "$lib/stores/accounts";
  import { trapFocus } from "$lib/utils/focusTrap";
  import { toast } from "$lib/stores/toast";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import QRCode from "qrcode";
  import ghostLogo from "$lib/assets/ghost.svg";
  import { getTheme } from "$lib/stores/theme.svelte";

  let { onclose, onsuccess }: { onclose: () => void; onsuccess: () => void } =
    $props();

  let session: SyncSessionInfo | null = $state(null);
  let status = $state("starting");
  let mergePreview: MergePreview | null = $state(null);
  let error = $state("");
  let loading = $state(false);
  let decisions: Map<string, string> = $state(new Map());
  let qrSvg = $state("");

  let mounted = $state(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    requestAnimationFrame(() => {
      mounted = true;
    });
  });

  $effect(() => {
    startSync();
    return () => stopPolling();
  });

  async function startSync() {
    try {
      session = await syncStart();
      qrSvg = await QRCode.toString(session.qr_data, {
        type: "svg",
        errorCorrectionLevel: "H",
        margin: 1,
        color: { dark: getTheme() === "dark" ? "#ffffff" : "#1a1a1a", light: "#00000000" },
      });
      status = "waiting";
      startPolling();
    } catch (e) {
      error = String(e);
      status = "error";
    }
  }

  function startPolling() {
    pollInterval = setInterval(async () => {
      try {
        const result = await syncPoll();
        status = result.status;
        if (result.status === "merge_ready" && result.merge_preview) {
          mergePreview = result.merge_preview;
          stopPolling();
          if (
            mergePreview.conflicts.length === 0 &&
            mergePreview.to_delete.length === 0
          ) {
            await handleAutoConfirm();
          }
        } else if (result.status === "error") {
          error = result.error || $_('syncInitiate.syncFailed');
          stopPolling();
        }
      } catch (e) {
        error = String(e);
        stopPolling();
      }
    }, 500);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
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
    stopPolling();
    try {
      await syncCancel();
    } catch {
      // ignore
    }
    close();
  }

  async function copyText(text: string) {
    try {
      await writeText(text);
      toast($_('syncInitiate.copied'));
    } catch {
      // silent
    }
  }

  function close() {
    stopPolling();
    mounted = false;
    setTimeout(onclose, 250);
  }

  function setDecision(id: string, action: string) {
    decisions = new Map(decisions);
    decisions.set(id, action);
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
    aria-labelledby="sync-initiate-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="flex items-center justify-between mb-6">
      <span id="sync-initiate-title" class="text-base tracking-wide text-muted"
        >{$_('syncInitiate.title')}</span
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
      <div
        class="border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-sm"
      >
        <span class="text-error/60">{$_('common.errorPrefix')}</span>
        {error}
      </div>
    {/if}

    {#if status === "starting"}
      <div class="text-center py-8">
        <p class="text-dim text-sm">{$_('syncInitiate.starting')}</p>
      </div>
    {:else if status === "waiting" && session}
      <div class="flex flex-col items-center gap-4">
        {#if qrSvg}
          <div class="relative w-52 h-52">
            <div class="w-full h-full qr-container">
              {@html qrSvg}
            </div>
            <div class="absolute inset-0 flex items-center justify-center">
              <div class="w-11 h-11 bg-bg rounded-sm flex items-center justify-center p-1.5">
                <img src={ghostLogo} alt="" class="w-full h-full icon-adapt opacity-60" />
              </div>
            </div>
          </div>
        {/if}

        <p class="text-sm text-dim">{$_('syncInitiate.manualCodeHint')}</p>

        <button
          type="button"
          class="w-full text-center border border-dotted border-border px-4 py-6 hover:border-fg/30 transition-colors"
          onclick={() => copyText(session!.text_code)}
        >
          <div
            class="text-xl tracking-[0.3em] text-fg font-mono leading-relaxed"
          >
            {session.text_code}
          </div>
          <div class="text-xs text-dim mt-2">{$_('syncInitiate.tapToCopy')}</div>
        </button>

        {#if session.all_hosts.length > 0}
          <div class="w-full border border-dotted border-border px-4 py-3">
            <div class="text-sm text-dim tracking-wide mb-2">
              {$_('syncInitiate.connection')}
            </div>
            {#each session.all_hosts as ip}
              <button
                type="button"
                class="w-full text-center hover:bg-fg/5 transition-colors py-1.5"
                onclick={() => copyText(`${ip}:${session!.port}`)}
              >
                <div class="text-base text-muted font-mono">
                  {ip}:{session.port}
                </div>
              </button>
            {/each}
            <div class="text-xs text-dim mt-2">{$_('syncInitiate.tapAddressToCopy')}</div>
          </div>
        {/if}

        <div class="flex items-center gap-2 text-dim text-sm">
          <span
            class="inline-block w-2 h-2 rounded-full bg-accent/60 animate-pulse"
          ></span>
          {$_('syncInitiate.waiting')}
        </div>
      </div>

      <div class="mt-6">
        <button
          type="button"
          class="w-full border border-dotted border-border text-dim text-sm py-2.5 hover:text-fg hover:border-fg/30 transition-colors"
          onclick={handleCancel}
        >
          {$_('common.cancel')}
        </button>
      </div>
    {:else if status === "exchanging"}
      <div class="text-center py-8">
        <div class="flex items-center justify-center gap-2 text-muted text-sm">
          <span
            class="inline-block w-2 h-2 rounded-full bg-accent animate-pulse"
          ></span>
          {$_('syncInitiate.syncing')}
        </div>
      </div>
    {:else if status === "merge_ready" && mergePreview}
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
            class="flex-1 border border-dotted border-border text-dim text-sm py-2.5 hover:text-fg hover:border-fg/30 transition-colors"
            onclick={handleCancel}
          >
            {$_('common.cancel')}
          </button>
          <button
            type="button"
            disabled={loading}
            class="flex-1 border border-fg/80 text-fg text-sm py-2.5 hover:bg-fg hover:text-bg transition-colors disabled:opacity-30"
            onclick={handleConfirm}
          >
            {loading ? $_('common.loading') : $_('sync.applySync')}
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .qr-container :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
