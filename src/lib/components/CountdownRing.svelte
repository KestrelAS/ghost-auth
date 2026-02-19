<script lang="ts">
  let { remaining, period = 30 }: { remaining: number; period?: number } = $props();

  let progress = $derived(remaining / period);
  let urgency = $derived(remaining <= 5 ? "critical" : remaining <= 10 ? "warning" : "normal");

  // Detect period reset to use a fast refill transition
  let prevRemaining = $state(remaining);
  let isRefilling = $state(false);
  $effect(() => {
    if (remaining > prevRemaining + 1) {
      isRefilling = true;
      setTimeout(() => { isRefilling = false; }, 200);
    }
    prevRemaining = remaining;
  });

  const size = 44;
  const stroke = 1.5;
  const radius = (size - stroke) / 2;
  const circumference = 2 * Math.PI * radius;
  let dashoffset = $derived(circumference * (1 - progress));
</script>

<div class="relative inline-flex items-center justify-center" style="width: {size}px; height: {size}px;">
  <svg width={size} height={size} viewBox="0 0 {size} {size}" class="absolute inset-0">
    <circle
      cx={size / 2}
      cy={size / 2}
      r={radius}
      fill="none"
      stroke="currentColor"
      stroke-width={stroke}
      class="opacity-10"
      stroke-dasharray="2 4"
    />
    <circle
      cx={size / 2}
      cy={size / 2}
      r={radius}
      fill="none"
      stroke-width={stroke}
      stroke-dasharray={circumference}
      stroke-dashoffset={dashoffset}
      stroke-linecap="butt"
      transform="rotate(-90 {size / 2} {size / 2})"
      class="transition-all ease-linear
        {isRefilling ? 'duration-200' : 'duration-1000'}
        {urgency === 'critical' ? 'stroke-error' : urgency === 'warning' ? 'stroke-fg/50' : 'stroke-fg/30'}"
    />
  </svg>
  <span
    class="text-sm font-medium tabular-nums
      {urgency === 'critical' ? 'text-error' : 'text-dim'}"
  >
    {remaining}
  </span>
</div>
