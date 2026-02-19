import { test, expect } from "@playwright/test";
import { mockTauriInvoke } from "./helpers";

test.describe("Ghost Auth E2E", () => {
  test("app loads without PIN and shows empty state", async ({ page }) => {
    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: [],
      generate_all_codes: [],
    });

    await page.goto("/");

    // Should show ghost auth header
    await expect(page.locator("text=ghost auth")).toBeVisible();

    // Should show [+add] button
    await expect(page.locator("text=[+add]")).toBeVisible();

    // Should show empty state message
    await expect(page.locator("text=no accounts found")).toBeVisible();
  });

  test("add account via manual entry", async ({ page }) => {
    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: [],
      generate_all_codes: [],
      add_account_manual: {
        id: "1",
        issuer: "GitHub",
        label: "user@test.com",
        algorithm: "SHA1",
        digits: 6,
        period: 30,
        icon: null,
      },
    });

    await page.goto("/");
    await page.click("text=[+add]");

    // Select manual entry
    await page.click("text=manual entry");

    // Fill form
    await page.fill("#issuer", "GitHub");
    await page.fill("#label", "user@test.com");
    await page.fill("#secret", "JBSWY3DPEHPK3PXP");

    // Override get_accounts to return the new account after add
    await page.evaluate(() => {
      const internals = (window as Record<string, unknown>)
        .__TAURI_INTERNALS__ as Record<string, unknown>;
      const originalInvoke = internals.invoke as Function;
      internals.invoke = async (cmd: string, args?: unknown) => {
        if (cmd === "get_accounts") {
          return [
            {
              id: "1",
              issuer: "GitHub",
              label: "user@test.com",
              algorithm: "SHA1",
              digits: 6,
              period: 30,
              icon: null,
            },
          ];
        }
        if (cmd === "generate_all_codes") {
          return [{ id: "1", code: "123456", remaining: 15 }];
        }
        return originalInvoke(cmd, args);
      };
    });

    await page.click("button:has-text('add')");

    // Should show the new account
    await expect(page.locator("text=GitHub")).toBeVisible({ timeout: 5000 });
  });

  test("PIN lock screen blocks access", async ({ page }) => {
    await mockTauriInvoke(page, {
      has_pin: true,
      verify_pin: true,
      get_accounts: [],
      generate_all_codes: [],
    });

    await page.goto("/");

    // Should show lock screen with PIN dots
    await expect(page.locator(".rounded-full").first()).toBeVisible();

    // Enter PIN via keyboard
    await page.keyboard.press("1");
    await page.keyboard.press("2");
    await page.keyboard.press("3");
    await page.keyboard.press("4");
    await page.keyboard.press("Enter");

    // After unlock, should see ghost auth header
    await expect(page.locator("text=ghost auth")).toBeVisible({
      timeout: 5000,
    });
  });

  test("settings panel toggles", async ({ page }) => {
    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: [],
      generate_all_codes: [],
    });

    await page.goto("/");

    // Click ghost auth header to toggle settings
    await page.click("text=ghost auth");

    // Should show pin lock toggle and backup buttons
    await expect(page.locator("text=pin lock")).toBeVisible();
    await expect(page.locator("text=export backup")).toBeVisible();
    await expect(page.locator("text=import backup")).toBeVisible();
  });

  test("wrong PIN shows error message", async ({ page }) => {
    await mockTauriInvoke(page, {
      has_pin: true,
      verify_pin: false,
      get_accounts: [],
      generate_all_codes: [],
    });

    await page.goto("/");

    // Enter wrong PIN
    await page.keyboard.press("1");
    await page.keyboard.press("2");
    await page.keyboard.press("3");
    await page.keyboard.press("4");
    await page.keyboard.press("Enter");

    // Should show incorrect pin error
    await expect(page.locator("text=incorrect pin")).toBeVisible({
      timeout: 5000,
    });
  });

  test("rate limiting shows lockout message", async ({ page }) => {
    await mockTauriInvoke(page, {
      has_pin: true,
      verify_pin: { error: "Too many attempts. Try again in 30 seconds." },
      get_accounts: [],
      generate_all_codes: [],
    });

    await page.goto("/");

    // Enter PIN (will trigger rate limit error)
    await page.keyboard.press("1");
    await page.keyboard.press("2");
    await page.keyboard.press("3");
    await page.keyboard.press("4");
    await page.keyboard.press("Enter");

    // Should show lockout message
    await expect(page.locator("text=too many attempts")).toBeVisible({
      timeout: 5000,
    });
  });

  test("multiple accounts display with codes", async ({ page }) => {
    const accounts = [
      { id: "1", issuer: "GitHub", label: "user@github.com", algorithm: "SHA1", digits: 6, period: 30, icon: null },
      { id: "2", issuer: "Google", label: "user@gmail.com", algorithm: "SHA1", digits: 6, period: 30, icon: null },
      { id: "3", issuer: "AWS", label: "admin@aws.com", algorithm: "SHA256", digits: 8, period: 30, icon: null },
    ];

    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: accounts,
      generate_all_codes: [
        { id: "1", code: "123456", remaining: 15 },
        { id: "2", code: "654321", remaining: 20 },
        { id: "3", code: "98765432", remaining: 10 },
      ],
    });

    await page.goto("/");

    // All accounts should be visible
    await expect(page.locator("text=GitHub")).toBeVisible();
    await expect(page.locator("text=Google")).toBeVisible();
    await expect(page.locator("text=AWS")).toBeVisible();

    // Codes should be displayed (formatted with spaces)
    await expect(page.locator("text=123 456")).toBeVisible();
    await expect(page.locator("text=654 321")).toBeVisible();
    await expect(page.locator("text=9876 5432")).toBeVisible();
  });

  test("search filters accounts by issuer", async ({ page }) => {
    const accounts = [
      { id: "1", issuer: "GitHub", label: "user@github.com", algorithm: "SHA1", digits: 6, period: 30, icon: null },
      { id: "2", issuer: "Google", label: "user@gmail.com", algorithm: "SHA1", digits: 6, period: 30, icon: null },
    ];

    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: accounts,
      generate_all_codes: [
        { id: "1", code: "123456", remaining: 15 },
        { id: "2", code: "654321", remaining: 20 },
      ],
    });

    await page.goto("/");

    // Both accounts visible initially
    await expect(page.locator("text=GitHub")).toBeVisible();
    await expect(page.locator("text=Google")).toBeVisible();

    // Search for "git"
    await page.fill('input[placeholder="> search accounts..."]', "git");

    // Only GitHub should remain visible
    await expect(page.locator("text=GitHub")).toBeVisible();
    await expect(page.locator("text=Google")).not.toBeVisible();
  });

  test("search shows no matches message", async ({ page }) => {
    const accounts = [
      { id: "1", issuer: "GitHub", label: "user@github.com", algorithm: "SHA1", digits: 6, period: 30, icon: null },
      { id: "2", issuer: "Google", label: "user@gmail.com", algorithm: "SHA1", digits: 6, period: 30, icon: null },
    ];

    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: accounts,
      generate_all_codes: [
        { id: "1", code: "123456", remaining: 15 },
        { id: "2", code: "654321", remaining: 20 },
      ],
    });

    await page.goto("/");

    // Search for something that matches nothing
    await page.fill('input[placeholder="> search accounts..."]', "zzzzz");

    // Should show no matches message
    await expect(page.locator('text=no matches for "zzzzz"')).toBeVisible();
  });

  test("PIN setup flow: enter, confirm, done", async ({ page }) => {
    let pinSetCalled = false;

    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: [],
      generate_all_codes: [],
      set_pin: null,
    });

    await page.goto("/");

    // Open settings and toggle PIN on
    await page.click("text=ghost auth");
    await expect(page.locator("text=pin lock")).toBeVisible();

    // Click the toggle to enable PIN
    await page.click('[aria-label="Toggle PIN lock"]');

    // Should show "choose a pin" prompt
    await expect(page.locator("text=choose a pin")).toBeVisible();

    // Enter PIN: 1234
    await page.keyboard.press("1");
    await page.keyboard.press("2");
    await page.keyboard.press("3");
    await page.keyboard.press("4");
    await page.keyboard.press("Enter");

    // Should show "confirm pin" prompt
    await expect(page.locator("text=confirm pin")).toBeVisible();

    // Confirm PIN: 1234
    await page.keyboard.press("1");
    await page.keyboard.press("2");
    await page.keyboard.press("3");
    await page.keyboard.press("4");
    await page.keyboard.press("Enter");

    // Should return to main screen (PIN setup complete)
    await expect(page.locator("text=ghost auth")).toBeVisible({
      timeout: 5000,
    });
  });

  test("PIN setup rejects mismatched confirmation", async ({ page }) => {
    await mockTauriInvoke(page, {
      has_pin: false,
      get_accounts: [],
      generate_all_codes: [],
      set_pin: null,
    });

    await page.goto("/");

    // Open settings and toggle PIN on
    await page.click("text=ghost auth");
    await page.click('[aria-label="Toggle PIN lock"]');

    // Enter PIN: 1234
    await page.keyboard.press("1");
    await page.keyboard.press("2");
    await page.keyboard.press("3");
    await page.keyboard.press("4");
    await page.keyboard.press("Enter");

    // Confirm with different PIN: 5678
    await page.keyboard.press("5");
    await page.keyboard.press("6");
    await page.keyboard.press("7");
    await page.keyboard.press("8");
    await page.keyboard.press("Enter");

    // Should show mismatch error and reset to "choose a pin"
    await expect(page.locator("text=pins don't match")).toBeVisible({
      timeout: 5000,
    });
  });
});
