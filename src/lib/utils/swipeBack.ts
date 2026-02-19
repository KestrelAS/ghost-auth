interface SwipeBackOptions {
  onclose: () => void;
  edgeWidth?: number;
  threshold?: number;
  velocityThreshold?: number;
}

export function swipeBack(node: HTMLElement, options: SwipeBackOptions) {
  const {
    onclose,
    edgeWidth = 25,
    threshold = 80,
    velocityThreshold = 0.5,
  } = options;

  let startX = 0;
  let startY = 0;
  let dx = 0;
  let active = false;
  let locked = false;
  let startTime = 0;

  // Find the backdrop sibling (previous sibling in DOM, or parent)
  function getBackdrop(): HTMLElement | null {
    return node.parentElement;
  }

  function onTouchStart(e: TouchEvent) {
    const touch = e.touches[0];
    if (touch.clientX > edgeWidth) return;
    if (e.touches.length > 1) return;

    startX = touch.clientX;
    startY = touch.clientY;
    dx = 0;
    active = true;
    locked = false;
    startTime = Date.now();

    // Disable CSS transition for real-time tracking
    node.style.transition = "none";
  }

  function onTouchMove(e: TouchEvent) {
    if (!active) return;
    const touch = e.touches[0];
    const currentDx = touch.clientX - startX;
    const currentDy = touch.clientY - startY;

    // Direction lock at 8px
    if (!locked && (Math.abs(currentDx) > 8 || Math.abs(currentDy) > 8)) {
      if (Math.abs(currentDy) > Math.abs(currentDx)) {
        // Vertical scroll — bail out
        active = false;
        node.style.transition = "";
        node.style.transform = "";
        return;
      }
      locked = true;
    }

    if (!locked) return;

    // Only allow swiping right (positive dx)
    dx = Math.max(0, currentDx);

    e.preventDefault();
    node.style.transform = `translateX(${dx}px)`;

    // Fade backdrop
    const backdrop = getBackdrop();
    if (backdrop) {
      const progress = Math.min(dx / window.innerWidth, 1);
      backdrop.style.opacity = String(1 - progress);
    }
  }

  function onTouchEnd() {
    if (!active) return;
    active = false;

    const elapsed = Date.now() - startTime;
    const velocity = elapsed > 0 ? dx / elapsed : 0;
    const shouldClose = dx > threshold || velocity > velocityThreshold;

    if (shouldClose) {
      // Animate out, then close
      node.style.transition = "transform 0.25s cubic-bezier(0.16, 1, 0.3, 1)";
      node.style.transform = "translateX(100%)";

      const backdrop = getBackdrop();
      if (backdrop) {
        backdrop.style.transition = "opacity 0.25s ease";
        backdrop.style.opacity = "0";
      }

      // Wait for animation, then call onclose and reset styles
      setTimeout(() => {
        node.style.transition = "";
        node.style.transform = "";
        if (backdrop) {
          backdrop.style.transition = "";
          backdrop.style.opacity = "";
        }
        onclose();
      }, 250);
    } else {
      // Snap back
      node.style.transition = "transform 0.3s cubic-bezier(0.16, 1, 0.3, 1)";
      node.style.transform = "translateX(0)";

      const backdrop = getBackdrop();
      if (backdrop) {
        backdrop.style.transition = "opacity 0.3s ease";
        backdrop.style.opacity = "";
      }

      const cleanup = () => {
        node.style.transition = "";
        node.style.transform = "";
        node.removeEventListener("transitionend", cleanup);
      };
      node.addEventListener("transitionend", cleanup, { once: true });
      // Safety fallback if transitionend doesn't fire
      setTimeout(cleanup, 350);
    }

    dx = 0;
    locked = false;
  }

  node.addEventListener("touchstart", onTouchStart, { passive: true });
  node.addEventListener("touchmove", onTouchMove, { passive: false });
  node.addEventListener("touchend", onTouchEnd);

  return {
    destroy() {
      node.removeEventListener("touchstart", onTouchStart);
      node.removeEventListener("touchmove", onTouchMove);
      node.removeEventListener("touchend", onTouchEnd);
    },
  };
}
