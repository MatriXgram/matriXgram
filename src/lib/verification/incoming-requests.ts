import { writable } from "svelte/store";

export interface IncomingVerificationRequest {
  userId: string;
  flowId: string;
  deviceId: string;
  read: boolean;
}

export function incomingVerificationRequestKey(request: Pick<IncomingVerificationRequest, "userId" | "flowId">): string {
  return `${request.userId}:${request.flowId}`;
}

export const incomingVerificationRequests = writable<IncomingVerificationRequest[]>([]);
export const activeIncomingVerificationRequest = writable<IncomingVerificationRequest | null>(null);

export function receiveIncomingVerificationRequest(request: Omit<IncomingVerificationRequest, "read">) {
  const key = incomingVerificationRequestKey(request);
  let notice: IncomingVerificationRequest = { ...request, read: false };
  let isNewRequest = false;
  incomingVerificationRequests.update((current) => {
    const existing = current.find((item) => incomingVerificationRequestKey(item) === key);
    if (existing) {
      notice = existing;
      return current;
    }
    isNewRequest = true;
    return [notice, ...current];
  });
  // Replayed sync events for an already-known flow should not reopen the same
  // modal after the user dismissed it. The notification can reopen it manually.
  if (isNewRequest) activeIncomingVerificationRequest.set(notice);
}

export function openIncomingVerificationRequest(request: IncomingVerificationRequest) {
  const key = incomingVerificationRequestKey(request);
  let opened = { ...request, read: true };
  incomingVerificationRequests.update((current) => current.map((item) => {
    if (incomingVerificationRequestKey(item) !== key) return item;
    opened = { ...item, read: true };
    return opened;
  }));
  activeIncomingVerificationRequest.set(opened);
}

export function closeIncomingVerificationRequest() {
  activeIncomingVerificationRequest.set(null);
}

export function resolveIncomingVerificationRequest(request: Pick<IncomingVerificationRequest, "userId" | "flowId">) {
  const key = incomingVerificationRequestKey(request);
  incomingVerificationRequests.update((current) => current.filter((item) => incomingVerificationRequestKey(item) !== key));
}

export function clearReadIncomingVerificationRequests() {
  incomingVerificationRequests.update((current) => current.filter((item) => !item.read));
}

export function clearIncomingVerificationRequests() {
  incomingVerificationRequests.set([]);
  activeIncomingVerificationRequest.set(null);
}
