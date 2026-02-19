<script lang="ts">
  import { toast } from "$lib/stores/toast";
  import { _ } from 'svelte-i18n';

  let { codes, ondone }: { codes: string[]; ondone: () => void } = $props();

  async function copyAll() {
    try {
      await navigator.clipboard.writeText(codes.join("\n"));
      toast($_('recoveryCodes.copied'));
    } catch {
      // Silent fail
    }
  }
</script>

<div class="fixed inset-0 z-50 bg-bg flex flex-col items-center justify-center select-none">
  <span class="text-base text-muted tracking-wide mb-3">{$_('recoveryCodes.title')}</span>

  <p class="text-sm text-dim max-w-xs text-center mb-8 px-4 leading-relaxed">
    {$_('recoveryCodes.description')}
  </p>

  <div class="grid grid-cols-2 gap-x-8 gap-y-3 mb-8">
    {#each codes as code, i}
      <div class="flex items-center gap-2">
        <span class="text-sm text-dim w-4 text-right">{i + 1}.</span>
        <span class="text-xl text-fg tracking-widest">{code}</span>
      </div>
    {/each}
  </div>

  <button
    type="button"
    class="text-sm text-dim border border-dotted border-border px-5 py-3 mb-4 hover:text-fg hover:border-fg/30 transition-colors"
    onclick={copyAll}
  >
    {$_('recoveryCodes.copyAll')}
  </button>

  <button
    type="button"
    class="text-sm text-fg border border-fg/80 px-8 py-3 hover:bg-fg hover:text-bg transition-colors"
    onclick={ondone}
  >
    {$_('recoveryCodes.saved')}
  </button>
</div>
