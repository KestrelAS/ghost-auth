<script lang="ts">
  import { _ } from 'svelte-i18n';
  import ghostLogo from "$lib/assets/ghost.svg";
  import AccountList from "$lib/components/AccountList.svelte";
  import AddAccount from "$lib/components/AddAccount.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";
  import PinSetup from "$lib/components/PinSetup.svelte";
  import PinRemove from "$lib/components/PinRemove.svelte";
  import BackupExport from "$lib/components/BackupExport.svelte";
  import BackupImport from "$lib/components/BackupImport.svelte";
  import ImportExternal from "$lib/components/ImportExternal.svelte";
  import SyncInitiate from "$lib/components/SyncInitiate.svelte";
  import SyncJoin from "$lib/components/SyncJoin.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import EditAccount from "$lib/components/EditAccount.svelte";
  import About from "$lib/components/About.svelte";
  import Help from "$lib/components/Help.svelte";
  import LanguageSelect from "$lib/components/LanguageSelect.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import ExportQr from "$lib/components/ExportQr.svelte";
  import BiometricPrompt from "$lib/components/BiometricPrompt.svelte";
  function cancelBarcodeScanner() {
    return import("@tauri-apps/plugin-barcode-scanner").then(m => m.cancel()).catch(() => {});
  }
  import {
    getAccounts,
    generateAllCodes,
    deleteAccount,
    reorderAccounts,
    hasPin,
    type AccountDisplay,
    type CodeResponse,
  } from "$lib/stores/accounts";

  let accounts: AccountDisplay[] = $state([]);
  let codes: Map<string, CodeResponse> = $state(new Map());
  let showAdd = $state(false);
  let error = $state("");

  // Lock state
  let locked = $state(true);
  let pinEnabled = $state(false);
  let loading = $state(true);
  let biometricHardwareAvailable = $state(false);
  let biometricEnabled = $state(false);
  let showPinSetup = $state(false);
  let showPinRemove = $state(false);
  let showSettings = $state(false);
  let showExport = $state(false);
  let showImport = $state(false);
  let showImportExternal = $state(false);
  let migrationData: number[] | undefined = $state(undefined);
  let showSyncInitiate = $state(false);
  let showSyncJoin = $state(false);
  let showAbout = $state(false);
  let showHelp = $state(false);
  let showLanguageSelect = $state(false);
  let showExportQr = $state(false);
  let showBiometricPrompt = $state(false);
  let codeRefreshFailures = $state(0);
  let appVisible = $state(true);

  // Back navigation history stack (Android system back button support)
  // Reactive so $effect can lock body scroll when overlays are open
  let overlayHistory: string[] = $state([]);
  let ignorePopstate = 0;
  let popstateClosing = false;

  function pushOverlay(name: string) {
    overlayHistory.push(name);
    history.pushState({ overlay: name }, '');
  }

  function removeOverlay(name: string) {
    const idx = overlayHistory.lastIndexOf(name);
    if (idx === -1) return;
    overlayHistory.splice(idx, 1);
    if (!popstateClosing) {
      ignorePopstate++;
      history.back();
    }
  }

  function removeMultipleOverlays(...names: string[]) {
    let count = 0;
    for (const name of names) {
      const idx = overlayHistory.lastIndexOf(name);
      if (idx !== -1) {
        overlayHistory.splice(idx, 1);
        count++;
      }
    }
    if (count > 0 && !popstateClosing) {
      ignorePopstate++;
      history.go(-count);
    }
  }

  function swapOverlay(oldName: string, newName: string) {
    const idx = overlayHistory.lastIndexOf(oldName);
    if (idx !== -1) {
      overlayHistory[idx] = newName;
      history.replaceState({ overlay: newName }, '');
    } else {
      pushOverlay(newName);
    }
  }

  function closeOverlayByName(name: string) {
    switch (name) {
      case 'settings':       showSettings = false; break;
      case 'add':            showAdd = false; break;
      case 'editAccount':    editingAccount = null; break;
      case 'pinSetup':       showPinSetup = false; break;
      case 'pinRemove':      showPinRemove = false; break;
      case 'export':         showExport = false; break;
      case 'import':         showImport = false; break;
      case 'importExternal': showImportExternal = false; migrationData = undefined; break;
      case 'syncInitiate':   showSyncInitiate = false; break;
      case 'syncJoin':       showSyncJoin = false; break;
      case 'about':          showAbout = false; break;
      case 'help':           showHelp = false; break;
      case 'languageSelect': showLanguageSelect = false; break;
      case 'exportQr':        showExportQr = false; break;
      case 'biometricPrompt': showBiometricPrompt = false; break;
      case 'scanning':       try { cancelBarcodeScanner().catch(() => {}); } catch {} break;
    }
  }

  function clearAllOverlays() {
    const count = overlayHistory.length;
    if (count > 0 && appVisible) {
      ignorePopstate++;
      history.go(-count);
    }
    overlayHistory = [];
    try { cancelBarcodeScanner().catch(() => {}); } catch {}
    showSettings = false;
    showAdd = false;
    editingAccount = null;
    showPinSetup = false;
    showPinRemove = false;
    showExport = false;
    showImport = false;
    showImportExternal = false;
    migrationData = undefined;
    showSyncInitiate = false;
    showSyncJoin = false;
    showExportQr = false;
    showAbout = false;
    showHelp = false;
    showLanguageSelect = false;
    showBiometricPrompt = false;
  }

  // Edit state
  let editingAccount: AccountDisplay | null = $state(null);

  // Search state
  let search = $state("");

  // Auto-lock after inactivity (5 minutes)
  const AUTO_LOCK_MS = 5 * 60 * 1000;
  let autoLockTimer: ReturnType<typeof setTimeout> | null = null;

  function resetAutoLock() {
    if (autoLockTimer) clearTimeout(autoLockTimer);
    if (!pinEnabled || locked || loading) return;
    autoLockTimer = setTimeout(() => {
      clearAllOverlays();
      locked = true;
    }, AUTO_LOCK_MS);
  }

  async function checkBiometricAvailability() {
    try {
      const { checkStatus } = await import("@tauri-apps/plugin-biometric");
      const status = await Promise.race([
        checkStatus(),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('timeout')), 3000)
        ),
      ]);
      biometricHardwareAvailable = status.isAvailable;
      const stored = localStorage.getItem('ghost-auth-biometric-enabled');
      biometricEnabled = stored === 'true' && biometricHardwareAvailable;
    } catch {
      biometricHardwareAvailable = false;
      biometricEnabled = false;
    }
  }

  function handleBiometricToggle() {
    biometricEnabled = !biometricEnabled;
    localStorage.setItem('ghost-auth-biometric-enabled', String(biometricEnabled));
  }

  async function checkPin() {
    try {
      pinEnabled = await hasPin();
      locked = pinEnabled;
      if (pinEnabled) {
        await checkBiometricAvailability();
      }
    } catch {
      locked = false;
      pinEnabled = false;
    } finally {
      loading = false;
    }
  }

  async function loadAccounts() {
    try {
      accounts = await getAccounts();
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshCodes() {
    if (accounts.length === 0) return;
    try {
      const allCodes = await generateAllCodes();
      const map = new Map<string, CodeResponse>();
      for (const c of allCodes) {
        map.set(c.id, c);
      }
      codes = map;
      codeRefreshFailures = 0;
    } catch {
      codeRefreshFailures++;
      if (codeRefreshFailures >= 3) {
        error = $_('app.refreshCodesFailed');
      }
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteAccount(id);
      await loadAccounts();
      await refreshCodes();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleAddSuccess() {
    showAdd = false;
    removeOverlay('add');
    await loadAccounts();
    await refreshCodes();
  }

  async function handleUnlock() {
    locked = false;
    pinEnabled = await hasPin();
    loadAccounts().then(() => refreshCodes());
  }

  function handlePinRemoved() {
    showPinRemove = false;
    pinEnabled = false;
    biometricEnabled = false;
    localStorage.removeItem('ghost-auth-biometric-enabled');
    showSettings = false;
    removeMultipleOverlays('pinRemove', 'settings');
  }

  async function handlePinSetupDone() {
    showPinSetup = false;
    pinEnabled = true;

    await checkBiometricAvailability();
    if (biometricHardwareAvailable && !biometricEnabled) {
      showBiometricPrompt = true;
      swapOverlay('pinSetup', 'biometricPrompt');
    } else {
      showSettings = false;
      removeMultipleOverlays('pinSetup', 'settings');
    }
  }

  function handleBiometricEnable() {
    biometricEnabled = true;
    localStorage.setItem('ghost-auth-biometric-enabled', 'true');
    showBiometricPrompt = false;
    showSettings = false;
    removeMultipleOverlays('biometricPrompt', 'settings');
  }

  function handleBiometricSkip() {
    biometricEnabled = false;
    localStorage.setItem('ghost-auth-biometric-enabled', 'false');
    showBiometricPrompt = false;
    showSettings = false;
    removeMultipleOverlays('biometricPrompt', 'settings');
  }

  function handleEdit(account: AccountDisplay) {
    editingAccount = account;
    pushOverlay('editAccount');
  }

  async function handleEditSuccess() {
    editingAccount = null;
    removeOverlay('editAccount');
    await loadAccounts();
    await refreshCodes();
  }

  async function handleReorder(ids: string[]) {
    try {
      await reorderAccounts(ids);
      await loadAccounts();
      await refreshCodes();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    checkPin();
  });

  // Lock immediately when the app goes to background (iOS / Android).
  // On iOS, JS timers freeze while suspended and all fire at once on
  // resume — locking here avoids the chaotic resume window entirely.
  $effect(() => {
    function handleVisibilityChange() {
      if (document.hidden) {
        appVisible = false;
        // Reset keyboard offset so elements aren't stuck mid-screen after unlock
        document.documentElement.style.setProperty('--keyboard-inset-bottom', '0px');
        if (pinEnabled && !locked && !loading) {
          clearAllOverlays();
          locked = true;
        }
      } else {
        // Delay re-enabling IPC traffic to let the WebView stabilize
        setTimeout(() => {
          appVisible = true;
        }, 500);
      }
    }
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  });

  // Android back button: close topmost overlay on popstate
  $effect(() => {
    function handlePopstate() {
      if (ignorePopstate > 0) {
        ignorePopstate--;
        return;
      }
      const top = overlayHistory.pop();
      if (top) {
        popstateClosing = true;
        closeOverlayByName(top);
        popstateClosing = false;
      }
    }
    window.addEventListener('popstate', handlePopstate);
    return () => window.removeEventListener('popstate', handlePopstate);
  });

  // Lock body scroll when any overlay is open
  $effect(() => {
    document.documentElement.style.overflow = overlayHistory.length > 0 ? 'hidden' : '';
  });

  // iOS keyboard handling: push modal panels above the virtual keyboard.
  // iOS WKWebView overlays the keyboard instead of resizing the viewport,
  // and visualViewport resize events may not fire, so we detect keyboard
  // via focus events on inputs inside modal panels and fall back to an
  // estimated height when viewport APIs don't report changes.
  // Android doesn't need this — its WebView resizes the viewport natively.
  $effect(() => {
    const isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) ||
                  (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
    if (!isIOS) return;

    const baseHeight = window.innerHeight;
    let focusTimer: ReturnType<typeof setTimeout> | null = null;
    let blurTimer: ReturnType<typeof setTimeout> | null = null;
    let viewportTimer: ReturnType<typeof setTimeout> | null = null;

    function getKeyboardHeight(): number {
      const vv = window.visualViewport;
      if (vv) {
        const offset = baseHeight - vv.height;
        if (offset > 100) return offset;
      }
      const diff = baseHeight - window.innerHeight;
      if (diff > 100) return diff;
      return 0;
    }

    function onFocusIn(e: FocusEvent) {
      const el = e.target as HTMLElement;
      if (el.tagName !== 'INPUT' && el.tagName !== 'TEXTAREA') return;
      if ((el as HTMLInputElement).type === 'file') return;
      if (!el.closest('.modal-panel') && !el.closest('.search-bottom')) return;

      if (blurTimer) { clearTimeout(blurTimer); blurTimer = null; }
      if (focusTimer) { clearTimeout(focusTimer); focusTimer = null; }

      focusTimer = setTimeout(() => {
        let kb = getKeyboardHeight();
        if (kb === 0) kb = Math.round(baseHeight * 0.4);
        if (kb > 0) {
          document.documentElement.style.setProperty('--keyboard-inset-bottom', `${kb}px`);
          el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
        }
        focusTimer = null;
      }, 20);
    }

    function onFocusOut() {
      if (focusTimer) { clearTimeout(focusTimer); focusTimer = null; }

      blurTimer = setTimeout(() => {
        const active = document.activeElement;
        if (active?.tagName !== 'INPUT' && active?.tagName !== 'TEXTAREA') {
          document.documentElement.style.setProperty('--keyboard-inset-bottom', '0px');
        }
        blurTimer = null;
      }, 150);
    }

    // Debounced: only update the CSS variable after the keyboard animation
    // settles, so we don't restart the CSS transition on every frame.
    function onViewportChange() {
      if (viewportTimer) clearTimeout(viewportTimer);
      viewportTimer = setTimeout(() => {
        const kb = getKeyboardHeight();
        if (kb > 100) {
          document.documentElement.style.setProperty('--keyboard-inset-bottom', `${kb}px`);
        } else if (kb === 0 && !focusTimer) {
          document.documentElement.style.setProperty('--keyboard-inset-bottom', '0px');
        }
        viewportTimer = null;
      }, 150);
    }

    document.addEventListener('focusin', onFocusIn);
    document.addEventListener('focusout', onFocusOut);
    const vv = window.visualViewport;
    if (vv) vv.addEventListener('resize', onViewportChange);
    window.addEventListener('resize', onViewportChange);

    return () => {
      if (focusTimer) clearTimeout(focusTimer);
      if (blurTimer) clearTimeout(blurTimer);
      if (viewportTimer) clearTimeout(viewportTimer);
      document.removeEventListener('focusin', onFocusIn);
      document.removeEventListener('focusout', onFocusOut);
      if (vv) vv.removeEventListener('resize', onViewportChange);
      window.removeEventListener('resize', onViewportChange);
      document.documentElement.style.removeProperty('--keyboard-inset-bottom');
    };
  });

  $effect(() => {
    if (locked || loading) return;
    loadAccounts().then(() => refreshCodes());

    const interval = setInterval(() => {
      if (!appVisible) return;
      refreshCodes();
    }, 1000);

    // Auto-lock on inactivity when PIN is enabled
    resetAutoLock();
    const activityEvents = ["pointerdown", "keydown"] as const;
    for (const evt of activityEvents) {
      window.addEventListener(evt, resetAutoLock);
    }

    return () => {
      clearInterval(interval);
      if (autoLockTimer) clearTimeout(autoLockTimer);
      for (const evt of activityEvents) {
        window.removeEventListener(evt, resetAutoLock);
      }
    };
  });
</script>

{#if loading}
  <div class="min-h-screen bg-bg flex flex-col items-center justify-center select-none splash-fade-in">
    <img src={ghostLogo} alt="" class="w-16 h-16 icon-adapt opacity-40 mb-6" />
    <h1 class="text-sm uppercase tracking-[0.25em] text-fg/80 mb-2">{$_('app.title')}</h1>
    <p class="text-[10px] uppercase tracking-[0.2em] text-dim">{$_('app.subtitle')}</p>
  </div>
{:else if locked && pinEnabled}
  <LockScreen onunlock={handleUnlock} {biometricEnabled} />
{:else}
  <main class="bg-bg text-fg fixed inset-0 flex flex-col">
    <!-- Header -->
    <header class="fixed top-0 left-0 right-0 z-10 bg-bg/90 backdrop-blur-sm border-dotted-b pt-safe">
      <div class="max-w-md mx-auto px-5 py-4 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <img src={ghostLogo} alt="" class="w-8 h-8 icon-adapt opacity-80" />
          <div class="flex flex-col">
            <span class="text-lg uppercase tracking-[0.2em] text-muted leading-none">{$_('app.headerTitle')}</span>
            <span class="text-[9px] uppercase tracking-[0.25em] text-dim leading-none mt-0.5">{$_('app.headerSubtitle')}</span>
          </div>
        </div>
        <button
          type="button"
          class="text-dim hover:text-fg transition-colors p-1"
          onclick={() => { showSettings = true; pushOverlay('settings'); }}
          aria-label={$_('app.settingsAriaLabel')}
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </button>
      </div>
    </header>

    <!-- Scrollable content area (explicit scroll container needed for iOS WKWebView) -->
    <div class="flex-1 overflow-y-auto min-h-0">
      <div class="pt-header"></div>

      <!-- Content -->
      <div class="max-w-md mx-auto px-5 pb-6 pt-2" style="padding-bottom: calc(env(safe-area-inset-bottom, 0px) + 6rem);">
        {#if error}
          <div class="border border-dotted border-error/30 text-error px-4 py-3 mb-5 text-xs">
            <span class="text-error/60">{$_('common.errorPrefix')}</span> {error}
          </div>
        {/if}

        <AccountList {accounts} {codes} ondelete={handleDelete} onedit={handleEdit} onreorder={handleReorder} {search} />
      </div>
    </div>

    <!-- Search bar (bottom, hovering) -->
    {#if accounts.length > 3}
      <div class="fixed left-6 z-20 h-16 flex items-center search-bottom" style="right: calc(1.5rem + 4rem + 1.5rem + 0.75rem);">
        <input
          type="text"
          bind:value={search}
          placeholder={$_('app.searchPlaceholder')}
          class="w-full bg-bg/60 backdrop-blur-md shadow-lg text-fg border border-dotted border-border px-3 py-2 text-sm outline-none focus:border-fg/40 transition-colors placeholder:text-dim"
        />
      </div>
    {/if}

    <!-- FAB: Add Account -->
    <button
      type="button"
      class="fixed right-6 w-16 h-16 rounded-full border-2 border-dotted border-dim bg-bg/60 backdrop-blur-md shadow-lg text-fg flex items-center justify-center text-2xl font-semibold z-20 hover:border-fg/30 active:scale-95 transition-all cursor-pointer fab-bottom"
      onclick={() => { showAdd = true; pushOverlay('add'); }}
      ontouchstart={() => {}}
      aria-label={$_('app.addAccountAriaLabel')}
    >
      +
    </button>

    <!-- Settings Slide-in -->
    {#if showSettings}
      <Settings
        onclose={() => { showSettings = false; removeOverlay('settings'); }}
        {pinEnabled}
        {biometricEnabled}
        {biometricHardwareAvailable}
        onbiometrictoggle={handleBiometricToggle}
        onpintoggle={() => { if (pinEnabled) { showPinRemove = true; pushOverlay('pinRemove'); } else { showPinSetup = true; pushOverlay('pinSetup'); } }}
        onexport={() => { showExport = true; pushOverlay('export'); }}
        onimport={() => { showImport = true; pushOverlay('import'); }}
        onimportexternal={() => { showImportExternal = true; pushOverlay('importExternal'); }}
        onexportqr={() => { showExportQr = true; pushOverlay('exportQr'); }}
        onsyncto={() => { showSyncJoin = true; pushOverlay('syncJoin'); }}
        onsyncfrom={() => { showSyncInitiate = true; pushOverlay('syncInitiate'); }}
        onabout={() => { showAbout = true; pushOverlay('about'); }}
        onhelp={() => { showHelp = true; pushOverlay('help'); }}
        onlanguage={() => { showLanguageSelect = true; pushOverlay('languageSelect'); }}
      />
    {/if}

    <!-- Add Account Modal -->
    {#if showAdd}
      <AddAccount onclose={() => { showAdd = false; removeOverlay('add'); }} onsuccess={handleAddSuccess} onmigration={(data) => { showAdd = false; migrationData = data; showImportExternal = true; swapOverlay('add', 'importExternal'); }} onimportexternal={() => { showAdd = false; showImportExternal = true; swapOverlay('add', 'importExternal'); }} onscanstart={() => pushOverlay('scanning')} onscanend={() => removeOverlay('scanning')} />
    {/if}

    <!-- Edit Account Modal -->
    {#if editingAccount}
      <EditAccount
        account={editingAccount}
        onclose={() => { editingAccount = null; removeOverlay('editAccount'); }}
        onsuccess={handleEditSuccess}
      />
    {/if}

    <!-- PIN Setup Modal -->
    {#if showPinSetup}
      <PinSetup onclose={() => { showPinSetup = false; removeOverlay('pinSetup'); }} ondone={handlePinSetupDone} />
    {/if}

    <!-- PIN Remove Modal -->
    {#if showPinRemove}
      <PinRemove onclose={() => { showPinRemove = false; removeOverlay('pinRemove'); }} ondone={handlePinRemoved} />
    {/if}

    <!-- Backup Modals -->
    {#if showExport}
      <BackupExport onclose={() => { showExport = false; removeOverlay('export'); }} />
    {/if}

    {#if showImport}
      <BackupImport onclose={() => { showImport = false; removeOverlay('import'); }} onsuccess={async () => { showImport = false; removeOverlay('import'); await loadAccounts(); await refreshCodes(); }} />
    {/if}

    {#if showImportExternal}
      <ImportExternal initialData={migrationData} onclose={() => { showImportExternal = false; migrationData = undefined; removeOverlay('importExternal'); }} onsuccess={async () => { showImportExternal = false; migrationData = undefined; removeOverlay('importExternal'); await loadAccounts(); await refreshCodes(); }} onscanstart={() => pushOverlay('scanning')} onscanend={() => removeOverlay('scanning')} />
    {/if}

    {#if showSyncInitiate}
      <SyncInitiate onclose={() => { showSyncInitiate = false; removeOverlay('syncInitiate'); }} onsuccess={async () => { showSyncInitiate = false; removeOverlay('syncInitiate'); await loadAccounts(); await refreshCodes(); }} />
    {/if}

    {#if showSyncJoin}
      <SyncJoin onclose={() => { showSyncJoin = false; removeOverlay('syncJoin'); }} onsuccess={async () => { showSyncJoin = false; removeOverlay('syncJoin'); await loadAccounts(); await refreshCodes(); }} onscanstart={() => pushOverlay('scanning')} onscanend={() => removeOverlay('scanning')} />
    {/if}

    <!-- Export QR Modal -->
    {#if showExportQr}
      <ExportQr onclose={() => { showExportQr = false; removeOverlay('exportQr'); }} />
    {/if}

    <!-- About Modal -->
    {#if showAbout}
      <About onclose={() => { showAbout = false; removeOverlay('about'); }} />
    {/if}

    <!-- Help Screen -->
    {#if showHelp}
      <Help onclose={() => { showHelp = false; removeOverlay('help'); }} />
    {/if}

    <!-- Language Select Screen -->
    {#if showLanguageSelect}
      <LanguageSelect onclose={() => { showLanguageSelect = false; removeOverlay('languageSelect'); }} />
    {/if}

    <!-- Biometric Prompt Modal -->
    {#if showBiometricPrompt}
      <BiometricPrompt
        onenable={handleBiometricEnable}
        onskip={handleBiometricSkip}
      />
    {/if}
  </main>
{/if}

<Toast />
