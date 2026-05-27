export type ToastTone = 'success' | 'info' | 'error';

export type ToastMessage = {
  id: number;
  tone: ToastTone;
  message: string;
};

export function toastToneForMessage(message: string, fallback: ToastTone = 'success'): ToastTone {
  if (/(unable|failed|error|blocked|denied|unavailable|not found|cannot|could not|requires|invalid|refusing)/i.test(message)) {
    return 'error';
  }
  if (/(saving|validating|loading|testing|waiting|restart)/i.test(message)) {
    return 'info';
  }
  return fallback;
}

export function toastDurationMs(tone: ToastTone): number {
  return tone === 'error' ? 6500 : 4200;
}

