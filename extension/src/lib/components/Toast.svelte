<script lang="ts">
  import { onToast } from "$lib/stores/toast";
  import CheckIcon from "./CheckIcon.svelte";

  let message = $state("");
  let visible = $state(false);
  let timer: ReturnType<typeof setTimeout>;

  onToast((msg) => {
    clearTimeout(timer);
    message = msg;
    visible = true;
    timer = setTimeout(() => { visible = false; }, 1800);
  });
</script>

<div
  class="fixed bottom-4 left-0 right-0 z-[999] flex justify-center pointer-events-none"
  aria-live="polite"
  role="status"
>
  <div
    class="toast-pill {visible ? 'open' : ''} flex items-center gap-2 bg-fg/10 backdrop-blur-md border border-dotted border-fg/10 px-4 py-2 rounded-full"
  >
    <span class="text-fg/60"><CheckIcon size={14} strokeWidth={2} animate={visible} color="currentColor" /></span>
    <span class="text-[11px] text-fg/70 tracking-wider">{message}</span>
  </div>
</div>

<style>
  .toast-pill {
    opacity: 0;
    transform: translateY(12px) scale(0.95);
    transition: opacity 0.2s ease, transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }
  .toast-pill.open {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
</style>
