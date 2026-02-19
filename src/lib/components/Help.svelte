<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { trapFocus } from "$lib/utils/focusTrap";
  import { swipeBack } from "$lib/utils/swipeBack";

  let { onclose }: { onclose: () => void } = $props();

  let mounted = $state(false);

  $effect(() => {
    requestAnimationFrame(() => { mounted = true; });
  });

  function close() {
    mounted = false;
    setTimeout(onclose, 300);
  }

  type Segment = { text: string; highlight: boolean };

  /** Parse translation strings that contain <span class="text-fg/80">...</span> into safe segments. */
  function richText(html: string): Segment[] {
    const re = /<span\s+class="text-fg\/80">(.*?)<\/span>/g;
    const parts: Segment[] = [];
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(html)) !== null) {
      if (m.index > last) parts.push({ text: html.slice(last, m.index), highlight: false });
      parts.push({ text: m[1], highlight: true });
      last = m.index + m[0].length;
    }
    if (last < html.length) parts.push({ text: html.slice(last), highlight: false });
    return parts;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-40 settings-backdrop {mounted ? 'open' : ''}"
  onclick={close}
  onkeydown={(e) => e.key === "Escape" && close()}
  role="presentation"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 bg-bg settings-slide {mounted ? 'open' : ''} flex flex-col pt-safe pb-safe"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="help-title"
    tabindex="-1"
    use:trapFocus
    use:swipeBack={{ onclose: () => setTimeout(onclose, 0) }}
  >
    <!-- Header -->
    <div class="max-w-md mx-auto w-full px-5 py-4 flex items-center gap-3 border-dotted-b">
      <button
        type="button"
        class="text-dim hover:text-fg transition-colors p-1"
        onclick={close}
        aria-label={$_('common.back')}
      >
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 4l-6 6 6 6" />
        </svg>
      </button>
      <span id="help-title" class="text-lg tracking-wide text-muted">{$_('help.title')}</span>
    </div>

    <!-- Content -->
    <div class="max-w-md mx-auto w-full px-5 py-6 flex flex-col gap-8 flex-1 overflow-y-auto">

      <!-- Intro -->
      <div>
        <h2 class="text-base tracking-wide text-fg/80 mb-1">{$_('help.introTitle')}</h2>
        <p class="text-xs text-dim leading-relaxed">{$_('help.introDesc')}</p>
      </div>

      <!-- Syncing -->
      <div>
        <div class="flex items-center gap-2 mb-3">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-accent opacity-70">
            <path d="M17 1l4 4-4 4" /><path d="M3 11V9a4 4 0 0 1 4-4h14" />
            <path d="M7 23l-4-4 4-4" /><path d="M21 13v2a4 4 0 0 1-4 4H3" />
          </svg>
          <p class="text-xs text-dim tracking-wide uppercase">{$_('help.syncTitle')}</p>
        </div>
        <div class="border border-dotted border-border px-4 py-3 flex flex-col gap-3">
          <p class="text-sm text-muted leading-relaxed">
            {$_('help.syncDesc')}
          </p>
          <p class="text-sm text-muted leading-relaxed">
            {#each richText($_('help.syncStep1')) as seg}{#if seg.highlight}<span class="text-fg/80">{seg.text}</span>{:else}{seg.text}{/if}{/each}
          </p>
          <p class="text-sm text-muted leading-relaxed">
            {#each richText($_('help.syncStep2')) as seg}{#if seg.highlight}<span class="text-fg/80">{seg.text}</span>{:else}{seg.text}{/if}{/each}
          </p>
          <div class="border-t border-dotted border-border pt-3">
            <p class="text-xs text-dim leading-relaxed">
              {$_('help.syncNote')}
            </p>
          </div>
        </div>
      </div>

      <!-- Importing from apps -->
      <div>
        <div class="flex items-center gap-2 mb-3">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-accent opacity-70">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
          <p class="text-xs text-dim tracking-wide uppercase">{$_('help.importTitle')}</p>
        </div>
        <div class="border border-dotted border-border px-4 py-3 flex flex-col gap-3">
          <p class="text-sm text-muted leading-relaxed">
            {$_('help.importDesc')}
          </p>
          <div>
            <p class="text-xs text-fg/60 tracking-wide mb-2">{$_('help.importStep1Title')}</p>
            <p class="text-sm text-muted leading-relaxed">
              {$_('help.importStep1')}
            </p>
          </div>
          <div>
            <p class="text-xs text-fg/60 tracking-wide mb-2">{$_('help.importStep2Title')}</p>
            <p class="text-sm text-muted leading-relaxed">
              {#each richText($_('help.importStep2')) as seg}{#if seg.highlight}<span class="text-fg/80">{seg.text}</span>{:else}{seg.text}{/if}{/each}
            </p>
          </div>
        </div>
      </div>

      <!-- Backups -->
      <div>
        <div class="flex items-center gap-2 mb-3">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-accent opacity-70">
            <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
            <polyline points="17 21 17 13 7 13 7 21" />
            <polyline points="7 3 7 8 15 8" />
          </svg>
          <p class="text-xs text-dim tracking-wide uppercase">{$_('help.backupTitle')}</p>
        </div>
        <div class="border border-dotted border-border px-4 py-3 flex flex-col gap-3">
          <div>
            <p class="text-xs text-fg/60 tracking-wide mb-2">{$_('help.backupExportTitle')}</p>
            <p class="text-sm text-muted leading-relaxed">
              {#each richText($_('help.backupExportDesc')) as seg}{#if seg.highlight}<span class="text-fg/80">{seg.text}</span>{:else}{seg.text}{/if}{/each}
            </p>
          </div>
          <div>
            <p class="text-xs text-fg/60 tracking-wide mb-2">{$_('help.backupImportTitle')}</p>
            <p class="text-sm text-muted leading-relaxed">
              {#each richText($_('help.backupImportDesc')) as seg}{#if seg.highlight}<span class="text-fg/80">{seg.text}</span>{:else}{seg.text}{/if}{/each}
            </p>
          </div>
          <div class="border-t border-dotted border-border pt-3">
            <p class="text-xs text-dim leading-relaxed">
              {$_('help.backupNote')}
            </p>
          </div>
        </div>
      </div>

      <!-- Losing access -->
      <div>
        <div class="flex items-center gap-2 mb-3">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-error opacity-70">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            <line x1="12" y1="9" x2="12" y2="13" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
          <p class="text-xs text-dim tracking-wide uppercase">{$_('help.accessTitle')}</p>
        </div>
        <div class="border border-dotted border-error/20 px-4 py-3 flex flex-col gap-3">
          <div>
            <p class="text-xs text-fg/60 tracking-wide mb-2">{$_('help.forgotPinTitle')}</p>
            <p class="text-sm text-muted leading-relaxed">
              {$_('help.forgotPinDesc1')}
            </p>
            <p class="text-sm text-muted leading-relaxed mt-2">
              {$_('help.forgotPinDesc2')}
            </p>
          </div>
          <div>
            <p class="text-xs text-fg/60 tracking-wide mb-2">{$_('help.lostDeviceTitle')}</p>
            <p class="text-sm text-muted leading-relaxed">
              {$_('help.lostDeviceDesc')}
            </p>
          </div>
          <div class="border-t border-dotted border-error/20 pt-3">
            <p class="text-xs text-error/70 leading-relaxed">
              {$_('help.accessWarning')}
            </p>
          </div>
        </div>
      </div>

      <!-- Bottom spacer -->
      <div class="pb-4"></div>
    </div>
  </div>
</div>

<style>
  .settings-backdrop {
    background: var(--color-backdrop-light);
    opacity: 0;
    transition: opacity 0.3s ease;
  }
  .settings-backdrop.open {
    opacity: 1;
  }
  .settings-slide {
    transform: translateX(100%);
    transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }
  .settings-slide.open {
    transform: translateX(0);
  }
</style>
