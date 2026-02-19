<script lang="ts">
  import "$lib/styles/app.css";
  import ghostLogo from "$lib/assets/ghost.svg";
  import {
    storage,
    getAccounts,
    getCodes,
    loadAccounts,
    refreshCodes,
    deleteAccount,
    reorderAccounts,
    type AccountDisplay,
  } from "$lib/stores/accounts.svelte";
  import AccountList from "$lib/components/AccountList.svelte";
  import AddAccount from "$lib/components/AddAccount.svelte";
  import EditAccount from "$lib/components/EditAccount.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import BackupExport from "$lib/components/BackupExport.svelte";
  import BackupImport from "$lib/components/BackupImport.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";
  import PinSetup from "$lib/components/PinSetup.svelte";
  import PinRemove from "$lib/components/PinRemove.svelte";
  import SyncConnect from "$lib/components/SyncConnect.svelte";
  import Help from "$lib/components/Help.svelte";
  import LanguageSelect from "$lib/components/LanguageSelect.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import { hasPin, verifyPin } from "$core/pin";
  import { STORAGE_KEYS } from "$core/constants";
  import { inputClass } from "$lib/styles/styles";
  import { _ } from 'svelte-i18n';

  // App state machine: loading → setup (first use) → locked → pinLocked → main
  type AppState = "loading" | "setup" | "locked" | "main";
  let state: AppState = $state("loading");

  // PIN lock (on top of master password unlock)
  let pinEnabled = $state(false);
  let pinLocked = $state(false);
  let pinColdLock = $state(false); // true when PIN needs to unwrap DEK (cold unlock)

  // Passwordless
  let passwordlessEnabled = $state(false);

  // Auto-lock
  let autoLockMinutes = $state(5);

  // Overlay state
  let showAdd = $state(false);
  let showSettings = $state(false);
  let showBackupExport = $state(false);
  let showBackupImport = $state(false);
  let showPinSetup = $state(false);
  let showPinRemove = $state(false);
  let showSync = $state(false);
  let showHelp = $state(false);
  let showLanguageSelect = $state(false);
  let editingAccount: AccountDisplay | null = $state(null);

  // Search
  let search = $state("");

  // Password fields
  let password = $state("");
  let confirmPassword = $state("");
  let passwordError = $state("");
  let passwordLoading = $state(false);

  // Code refresh interval
  let refreshInterval: ReturnType<typeof setInterval> | null = null;

  let accounts = $derived(getAccounts());
  let codes = $derived(getCodes());

  const browserRuntime = (globalThis as any).browser?.runtime ?? (globalThis as any).chrome?.runtime;

  // Initialize on mount
  $effect(() => {
    init();
    return () => {
      if (refreshInterval) clearInterval(refreshInterval);
    };
  });

  async function init() {
    try {
      const initialized = await storage.isInitialized();
      if (!initialized) {
        state = "setup";
        return;
      }

      // Try to restore session (DEK cached in chrome.storage.session)
      const restored = await storage.tryRestoreSession();
      if (restored) {
        await loadAccounts();
        pinEnabled = await hasPin();
        passwordlessEnabled = await storage.isPasswordless();
        if (pinEnabled) {
          pinLocked = true;
          pinColdLock = false; // warm — session is active, just verify PIN
          state = "main";
        } else {
          state = "main";
          startRefresh();
          resetAutoLock();
        }
        loadAutoLockSetting();
        return;
      }

      // Session expired — check alternative unlock methods

      // Passwordless: auto-restore DEK without credentials
      if (await storage.isPasswordless()) {
        const ok = await storage.tryPasswordlessRestore();
        if (ok) {
          await loadAccounts();
          passwordlessEnabled = true;
          pinEnabled = false;
          state = "main";
          startRefresh();
          resetAutoLock();
          loadAutoLockSetting();
          return;
        }
      }

      // PIN-wrapped DEK: show PIN screen for cold unlock
      if (await storage.hasPinWrappedDek()) {
        pinEnabled = true;
        pinLocked = true;
        pinColdLock = true;
        state = "main";
        loadAutoLockSetting();
        return;
      }

      // Fallback: master password
      state = "locked";
    } catch (e) {
      console.error("Init failed:", e);
      state = "setup";
    }
  }

  async function loadAutoLockSetting() {
    try {
      if (browserRuntime) {
        const resp = await browserRuntime.sendMessage({ type: "get-auto-lock-timeout" });
        if (resp?.minutes !== undefined) autoLockMinutes = resp.minutes;
      }
    } catch {
      // Service worker might not be available
    }
  }

  async function handleSetup() {
    passwordError = "";
    if (password.length < 8) {
      passwordError = $_('ext.setup.passwordTooShort');
      return;
    }
    if (password !== confirmPassword) {
      passwordError = $_('ext.setup.passwordMismatch');
      return;
    }
    passwordLoading = true;
    try {
      await storage.initialize(password);
      await loadAccounts();
      password = "";
      confirmPassword = "";
      state = "main";
      startRefresh();
      resetAutoLock();
    } catch (e) {
      passwordError = String(e);
    } finally {
      passwordLoading = false;
    }
  }

  async function handleUnlock() {
    passwordError = "";
    if (!password) {
      passwordError = $_('ext.unlock.enterPassword');
      return;
    }
    passwordLoading = true;
    try {
      const ok = await storage.unlock(password);
      if (!ok) {
        passwordError = $_('ext.unlock.wrongPassword');
        passwordLoading = false;
        return;
      }
      await loadAccounts();
      password = "";
      pinEnabled = await hasPin();
      passwordlessEnabled = await storage.isPasswordless();
      state = "main";
      startRefresh();
      resetAutoLock();
      loadAutoLockSetting();
    } catch (e) {
      passwordError = String(e);
    } finally {
      passwordLoading = false;
    }
  }

  function handlePinUnlock() {
    pinLocked = false;
    pinColdLock = false;
    startRefresh();
    resetAutoLock();
  }

  async function handlePinColdUnlock(pin: string): Promise<boolean> {
    // Verify PIN hash first (enforces rate limiting)
    const pinOk = await verifyPin(pin);
    if (!pinOk) return false;
    // Unwrap DEK using PIN-derived key
    const dekOk = await storage.unwrapDekWithPin(pin);
    if (dekOk) {
      await loadAccounts();
      passwordlessEnabled = await storage.isPasswordless();
    }
    return dekOk;
  }

  function handlePasswordFallback() {
    pinLocked = false;
    pinColdLock = false;
    state = "locked";
  }

  function handleLock() {
    if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = null;
    }
    storage.lock();
    clearAutoLock();
    closeAllOverlays();
    state = "locked";
    password = "";
    passwordError = "";
    pinLocked = false;
    pinColdLock = false;
    // Re-init to route to correct unlock screen (PIN/passwordless/password)
    init();
  }

  function startRefresh() {
    if (refreshInterval) clearInterval(refreshInterval);
    refreshInterval = setInterval(() => {
      refreshCodes();
    }, 1000);
  }

  // ── Auto-lock ──

  function resetAutoLock() {
    try {
      browserRuntime?.sendMessage({ type: "reset-auto-lock" });
    } catch {
      // Ignore if service worker not available
    }
  }

  function clearAutoLock() {
    try {
      browserRuntime?.sendMessage({ type: "clear-auto-lock" });
    } catch {
      // Ignore
    }
  }

  async function handleAutoLockChange(minutes: number) {
    autoLockMinutes = minutes;
    try {
      if (browserRuntime) {
        await browserRuntime.sendMessage({ type: "set-auto-lock-timeout", minutes });
      }
    } catch {
      // Ignore
    }
  }

  // ── Overlay helpers ──

  function closeAllOverlays() {
    showAdd = false;
    showSettings = false;
    showBackupExport = false;
    showBackupImport = false;
    showPinSetup = false;
    showPinRemove = false;
    showSync = false;
    showHelp = false;
    showLanguageSelect = false;
    editingAccount = null;
  }

  // ── Account handlers ──

  async function handleDelete(id: string) {
    await deleteAccount(id);
  }

  function handleEdit(account: AccountDisplay) {
    editingAccount = account;
  }

  async function handleReorder(ids: string[]) {
    await reorderAccounts(ids);
  }

  function handleAddSuccess() {
    showAdd = false;
    refreshCodes();
  }

  function handleEditSuccess() {
    editingAccount = null;
  }

  // ── PIN handlers ──

  function handlePinToggle() {
    if (pinEnabled) {
      showPinRemove = true;
    } else {
      showPinSetup = true;
    }
  }

  async function handlePinSetupDone() {
    showPinSetup = false;
    pinEnabled = true;
    // PIN and passwordless are mutually exclusive
    if (passwordlessEnabled) {
      await storage.setPasswordless(false);
      passwordlessEnabled = false;
    }
    showSettings = false;
  }

  function handlePinRemoved() {
    showPinRemove = false;
    pinEnabled = false;
    showSettings = false;
  }

  // ── Passwordless handler ──

  async function handlePasswordlessToggle(enabled: boolean) {
    if (enabled) {
      await storage.setPasswordless(true);
      passwordlessEnabled = true;
      // Passwordless and PIN are mutually exclusive
      if (pinEnabled) {
        try {
          const bs = (globalThis as any).browser?.storage ?? chrome.storage;
          await bs.local.remove([STORAGE_KEYS.pinHash, STORAGE_KEYS.pinRateLimit, "ghost_pin_recovery"]);
          await storage.clearPinWrappedDek();
        } catch { /* ignore */ }
        pinEnabled = false;
      }
    } else {
      await storage.setPasswordless(false);
      passwordlessEnabled = false;
    }
  }
</script>

<div class="min-h-[540px] flex flex-col">
  {#if state === "loading"}
    <!-- Loading splash -->
    <div class="flex-1 flex flex-col items-center justify-center splash-fade-in">
      <img src={ghostLogo} alt="Ghost Auth" class="w-16 h-16 icon-adapt opacity-30" />
    </div>

  {:else if state === "setup"}
    <!-- First-time setup -->
    <div class="flex-1 flex flex-col items-center justify-center px-6">
      <img src={ghostLogo} alt="Ghost Auth" class="w-12 h-12 icon-adapt opacity-40 mb-6" />
      <h1 class="text-base text-muted tracking-wide mb-1">{$_('ext.setup.heading')}</h1>
      <p class="text-xs text-dim mb-6">{$_('ext.setup.description')}</p>

      {#if passwordError}
        <div class="w-full border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-xs animate-shake">
          <span class="text-error/60">{$_('common.errorPrefix')}</span> {passwordError}
        </div>
      {/if}

      <form
        class="w-full flex flex-col gap-3"
        onsubmit={(e) => { e.preventDefault(); handleSetup(); }}
      >
        <input
          type="password"
          bind:value={password}
          placeholder={$_('ext.setup.passwordPlaceholder')}
          class={inputClass}
          autocomplete="new-password"
        />
        <input
          type="password"
          bind:value={confirmPassword}
          placeholder={$_('ext.setup.confirmPlaceholder')}
          class={inputClass}
          autocomplete="new-password"
        />
        <button
          type="submit"
          disabled={passwordLoading}
          class="w-full border border-fg/80 text-fg text-sm py-2.5 hover:bg-fg hover:text-bg transition-colors disabled:opacity-30 mt-2"
        >
          {passwordLoading ? $_('ext.setup.encrypting') : $_('ext.setup.createVault')}
        </button>
      </form>
    </div>

  {:else if state === "locked"}
    <!-- Unlock screen -->
    <div class="flex-1 flex flex-col items-center justify-center px-6">
      <img src={ghostLogo} alt="Ghost Auth" class="w-12 h-12 icon-adapt opacity-40 mb-6" />
      <h1 class="text-base text-muted tracking-wide mb-6">{$_('ext.unlock.title')}</h1>

      {#if passwordError}
        <div class="w-full border border-dotted border-error/30 text-error px-3 py-2 mb-4 text-xs animate-shake">
          <span class="text-error/60">{$_('common.errorPrefix')}</span> {passwordError}
        </div>
      {/if}

      <form
        class="w-full flex flex-col gap-3"
        onsubmit={(e) => { e.preventDefault(); handleUnlock(); }}
      >
        <input
          type="password"
          bind:value={password}
          placeholder={$_('ext.unlock.placeholder')}
          class={inputClass}
          autocomplete="current-password"
        />
        <button
          type="submit"
          disabled={passwordLoading}
          class="w-full border border-fg/80 text-fg text-sm py-2.5 hover:bg-fg hover:text-bg transition-colors disabled:opacity-30"
        >
          {passwordLoading ? $_('ext.unlock.decrypting') : $_('ext.unlock.unlock')}
        </button>
      </form>
    </div>

  {:else}
    <!-- Main view (may have PIN lock overlay on top) -->
    {#if pinLocked}
      <LockScreen
        onunlock={handlePinUnlock}
        onsubmitpin={pinColdLock ? handlePinColdUnlock : undefined}
        onpasswordfallback={pinColdLock ? handlePasswordFallback : undefined}
        onrecoveryused={pinColdLock ? handlePasswordFallback : undefined}
      />
    {:else}
      <!-- Header -->
      <div class="flex items-center justify-between px-4 py-3 border-dotted-b">
        <div class="flex items-center gap-2">
          <img src={ghostLogo} alt="" class="w-5 h-5 icon-adapt opacity-60" />
          <span class="text-xs text-dim tracking-widest uppercase">{$_('ext.header.title')}</span>
        </div>
        <button
          type="button"
          class="text-dim hover:text-fg transition-colors p-1"
          onclick={() => showSettings = true}
          aria-label={$_('app.settingsAriaLabel')}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round">
            <circle cx="8" cy="8" r="2.5" />
            <path d="M6.5 1.5h3l.4 1.8a5.5 5.5 0 0 1 1.3.7l1.7-.6 1.5 2.6-1.3 1.2a5.5 5.5 0 0 1 0 1.6l1.3 1.2-1.5 2.6-1.7-.6a5.5 5.5 0 0 1-1.3.7l-.4 1.8h-3l-.4-1.8a5.5 5.5 0 0 1-1.3-.7l-1.7.6-1.5-2.6 1.3-1.2a5.5 5.5 0 0 1 0-1.6L2.1 6l1.5-2.6 1.7.6a5.5 5.5 0 0 1 1.3-.7z" />
          </svg>
        </button>
      </div>

      <!-- Search bar (shown when > 3 accounts) -->
      {#if accounts.length > 3}
        <div class="px-4 py-2 border-dotted-b">
          <input
            type="text"
            bind:value={search}
            placeholder={$_('app.searchPlaceholder')}
            class="w-full bg-transparent text-fg text-xs outline-none placeholder:text-dim"
          />
        </div>
      {/if}

      <!-- Account list -->
      <div class="flex-1 overflow-y-auto px-4">
        <AccountList
          {accounts}
          {codes}
          ondelete={handleDelete}
          onedit={handleEdit}
          onreorder={handleReorder}
          {search}
        />
      </div>

      <!-- Add button -->
      <div class="px-4 py-3 border-dotted-t">
        <button
          type="button"
          class="w-full border border-dotted border-border text-dim text-xs py-2 hover:text-fg hover:border-fg/30 transition-colors tracking-wider"
          onclick={() => showAdd = true}
        >
          {$_('ext.accountList.addAccount')}
        </button>
      </div>
    {/if}
  {/if}

  <!-- Overlays -->
  {#if showAdd}
    <AddAccount onclose={() => showAdd = false} onsuccess={handleAddSuccess} />
  {/if}

  {#if editingAccount}
    <EditAccount
      account={editingAccount}
      onclose={() => editingAccount = null}
      onsuccess={handleEditSuccess}
    />
  {/if}

  {#if showSettings}
    <Settings
      onclose={() => showSettings = false}
      onlock={handleLock}
      onexport={() => showBackupExport = true}
      onimport={() => showBackupImport = true}
      onpintoggle={handlePinToggle}
      onsync={() => showSync = true}
      onhelp={() => showHelp = true}
      onlanguage={() => { showSettings = false; setTimeout(() => { showLanguageSelect = true; }, 300); }}
      {pinEnabled}
      {autoLockMinutes}
      onautolockchange={handleAutoLockChange}
      {passwordlessEnabled}
      onpasswordlesstoggle={handlePasswordlessToggle}
    />
  {/if}

  {#if showBackupExport}
    <BackupExport onclose={() => showBackupExport = false} />
  {/if}

  {#if showBackupImport}
    <BackupImport
      onclose={() => showBackupImport = false}
      onsuccess={() => { showBackupImport = false; refreshCodes(); }}
    />
  {/if}

  {#if showPinSetup}
    <PinSetup
      onclose={() => showPinSetup = false}
      ondone={handlePinSetupDone}
      onwrapdek={(pin: string) => storage.wrapDekWithPin(pin)}
    />
  {/if}

  {#if showPinRemove}
    <PinRemove onclose={() => showPinRemove = false} ondone={handlePinRemoved} />
  {/if}

  {#if showSync}
    <SyncConnect
      onclose={() => showSync = false}
      onsuccess={() => { showSync = false; refreshCodes(); }}
    />
  {/if}

  {#if showHelp}
    <Help onclose={() => showHelp = false} />
  {/if}

  {#if showLanguageSelect}
    <LanguageSelect onclose={() => showLanguageSelect = false} />
  {/if}

  <Toast />
</div>
