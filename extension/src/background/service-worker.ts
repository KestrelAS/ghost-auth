const AUTO_LOCK_ALARM = "ghost-auth-auto-lock";
const DEFAULT_AUTO_LOCK_MINUTES = 5;
const AUTO_LOCK_SETTING_KEY = "ghost_auto_lock_minutes";

const browserRuntime = (globalThis as any).browser?.runtime ?? chrome.runtime;
const browserAlarms = (globalThis as any).browser?.alarms ?? chrome.alarms;
const browserStorage = (globalThis as any).browser?.storage ?? chrome.storage;

// Auto-lock: clear cached DEK from session storage
browserAlarms.onAlarm.addListener((alarm: chrome.alarms.Alarm) => {
  if (alarm.name === AUTO_LOCK_ALARM) {
    browserStorage.session?.remove("ghost_dek").catch(() => {});
  }
});

// Listen for messages from popup
browserRuntime.onMessage.addListener(
  (msg: { type: string; minutes?: number }, _sender: unknown, sendResponse: (resp: unknown) => void) => {
    if (msg.type === "reset-auto-lock") {
      // Get configured timeout or use default
      browserStorage.local.get(AUTO_LOCK_SETTING_KEY).then((result: Record<string, unknown>) => {
        const minutes = (result[AUTO_LOCK_SETTING_KEY] as number) || DEFAULT_AUTO_LOCK_MINUTES;
        browserAlarms.clear(AUTO_LOCK_ALARM);
        if (minutes > 0) {
          browserAlarms.create(AUTO_LOCK_ALARM, { delayInMinutes: minutes });
        }
        sendResponse({ ok: true });
      }).catch(() => {
        sendResponse({ ok: false });
      });
      return true; // async response
    } else if (msg.type === "clear-auto-lock") {
      browserAlarms.clear(AUTO_LOCK_ALARM);
      sendResponse({ ok: true });
    } else if (msg.type === "set-auto-lock-timeout") {
      const minutes = msg.minutes ?? DEFAULT_AUTO_LOCK_MINUTES;
      browserStorage.local.set({ [AUTO_LOCK_SETTING_KEY]: minutes }).then(() => {
        // Reset the alarm with the new timeout
        browserAlarms.clear(AUTO_LOCK_ALARM);
        if (minutes > 0) {
          browserAlarms.create(AUTO_LOCK_ALARM, { delayInMinutes: minutes });
        }
        sendResponse({ ok: true });
      }).catch(() => {
        sendResponse({ ok: false });
      });
      return true; // async response
    } else if (msg.type === "get-auto-lock-timeout") {
      browserStorage.local.get(AUTO_LOCK_SETTING_KEY).then((result: Record<string, unknown>) => {
        sendResponse({ minutes: (result[AUTO_LOCK_SETTING_KEY] as number) || DEFAULT_AUTO_LOCK_MINUTES });
      }).catch(() => {
        sendResponse({ minutes: DEFAULT_AUTO_LOCK_MINUTES });
      });
      return true; // async response
    }
    return false;
  },
);
