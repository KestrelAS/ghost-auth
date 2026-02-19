<script lang="ts">
  import { _ } from 'svelte-i18n';
  import jsQR from "jsqr";

  let { onscan, oncancel }: {
    onscan: (content: string) => void;
    oncancel: () => void;
  } = $props();

  let videoEl: HTMLVideoElement | undefined = $state(undefined);
  let canvasEl: HTMLCanvasElement | undefined = $state(undefined);
  let error = $state("");
  let stream: MediaStream | null = null;
  let scanTimer: ReturnType<typeof setTimeout> | null = null;
  let stopped = false;

  $effect(() => {
    startCamera();
    return () => stopCamera();
  });

  async function startCamera() {
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: "environment", width: { ideal: 1280 }, height: { ideal: 720 } },
      });
      if (stopped) {
        stream.getTracks().forEach((t) => t.stop());
        return;
      }
      if (videoEl) {
        videoEl.srcObject = stream;
        await videoEl.play();
        scanLoop();
      }
    } catch (e: unknown) {
      const name = e instanceof Error ? e.name : "";
      if (name === "NotAllowedError") {
        error = $_('webScanner.permissionDenied');
      } else if (name === "NotFoundError" || name === "OverconstrainedError") {
        error = $_('webScanner.noCamera');
      } else {
        error = $_('webScanner.accessFailed');
      }
    }
  }

  function scanLoop() {
    if (stopped || !videoEl || !canvasEl || videoEl.readyState < 2) {
      if (!stopped) scanTimer = setTimeout(scanLoop, 100);
      return;
    }

    const ctx = canvasEl.getContext("2d", { willReadFrequently: true });
    if (!ctx) return;

    canvasEl.width = videoEl.videoWidth;
    canvasEl.height = videoEl.videoHeight;
    ctx.drawImage(videoEl, 0, 0);

    const imageData = ctx.getImageData(0, 0, canvasEl.width, canvasEl.height);
    const qr = jsQR(imageData.data, imageData.width, imageData.height);

    if (qr && qr.data) {
      stopCamera();
      onscan(qr.data);
      return;
    }

    scanTimer = setTimeout(scanLoop, 100);
  }

  function stopCamera() {
    stopped = true;
    if (scanTimer) {
      clearTimeout(scanTimer);
      scanTimer = null;
    }
    if (stream) {
      stream.getTracks().forEach((t) => t.stop());
      stream = null;
    }
  }

  function handleCancel() {
    stopCamera();
    oncancel();
  }
</script>

<div class="fixed inset-0 z-[100] bg-bg flex flex-col items-center justify-center">
  <button
    type="button"
    class="absolute right-5 top-4 w-10 h-10 flex items-center justify-center bg-fg/10 hover:bg-fg/20 transition-colors rounded-full z-10"
    onclick={handleCancel}
    aria-label={$_('scanner.closeCamera')}
  >
    <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" class="text-fg">
      <line x1="2" y1="2" x2="12" y2="12" /><line x1="12" y1="2" x2="2" y2="12" />
    </svg>
  </button>

  {#if error}
    <div class="text-error text-sm px-6 text-center">{error}</div>
  {:else}
    <div class="relative w-full max-w-sm aspect-square overflow-hidden rounded">
      <!-- svelte-ignore element_invalid_self_closing_tag -->
      <video bind:this={videoEl} class="w-full h-full object-cover" playsinline muted />
      <div class="absolute inset-4 pointer-events-none">
        <div class="absolute top-0 left-0 w-8 h-8 border-t-[3px] border-l-[3px] border-fg/80"></div>
        <div class="absolute top-0 right-0 w-8 h-8 border-t-[3px] border-r-[3px] border-fg/80"></div>
        <div class="absolute bottom-0 left-0 w-8 h-8 border-b-[3px] border-l-[3px] border-fg/80"></div>
        <div class="absolute bottom-0 right-0 w-8 h-8 border-b-[3px] border-r-[3px] border-fg/80"></div>
      </div>
    </div>
    <p class="text-xs text-dim mt-4 tracking-wide">{$_('webScanner.instruction')}</p>
  {/if}

  <canvas bind:this={canvasEl} class="hidden"></canvas>
</div>
