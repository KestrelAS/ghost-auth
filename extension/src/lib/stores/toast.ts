let listener: ((msg: string) => void) | null = null;

export function onToast(fn: (msg: string) => void) {
  listener = fn;
}

export function toast(msg: string) {
  listener?.(msg);
}
