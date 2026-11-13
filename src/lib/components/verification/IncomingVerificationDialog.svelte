<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    matrixAcceptSasVerification,
    matrixAcceptVerificationRequest,
    matrixConfirmSasVerification,
    matrixGetVerificationFlow,
  } from "$lib/chats/api";
  import type { MatrixVerificationFlowResponse } from "$lib/chats/types";
  import { EVENT_VERIFICATION_REQUESTED } from "$lib/events";

  let flow = $state<MatrixVerificationFlowResponse | null>(null);
  let deviceId = $state("");
  let pending = $state(false);
  let error = $state("");
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  let requestKey = "";

  onMount(() => {
    let unlisten = () => {};
    void listen<{ userId: string; flowId: string; deviceId: string }>(
      EVENT_VERIFICATION_REQUESTED,
      ({ payload }) => {
        const key = `${payload.userId}:${payload.flowId}`;
        if (key === requestKey) return;
        requestKey = key;
        deviceId = payload.deviceId;
        error = "";
        flow = null;
        void loadFlow(payload.userId, payload.flowId);
      },
    ).then((stop) => { unlisten = stop; });

    return () => {
      unlisten();
      if (pollTimer) clearTimeout(pollTimer);
    };
  });

  async function loadFlow(userId: string, flowId: string) {
    try {
      flow = await matrixGetVerificationFlow(userId, flowId);
      scheduleRefresh(userId, flowId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Could not load verification request";
    }
  }

  function scheduleRefresh(userId: string, flowId: string) {
    if (pollTimer) clearTimeout(pollTimer);
    if (!flow || flow.isDone || flow.isCancelled) return;
    pollTimer = setTimeout(async () => {
      try {
        flow = await matrixGetVerificationFlow(userId, flowId);
        scheduleRefresh(userId, flowId);
      } catch (cause) {
        error = cause instanceof Error ? cause.message : "Could not refresh verification";
      }
    }, 900);
  }

  async function act(action: "accept" | "acceptSas" | "confirm") {
    if (!flow || pending) return;
    pending = true;
    error = "";
    try {
      if (action === "accept") flow = await matrixAcceptVerificationRequest(flow.userId, flow.flowId);
      else if (action === "acceptSas") flow = await matrixAcceptSasVerification(flow.userId, flow.flowId);
      else flow = await matrixConfirmSasVerification(flow.userId, flow.flowId);
      scheduleRefresh(flow.userId, flow.flowId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Verification action failed";
    } finally {
      pending = false;
    }
  }

  function close() {
    flow = null;
    deviceId = "";
    requestKey = "";
    error = "";
    if (pollTimer) clearTimeout(pollTimer);
  }
</script>

{#if flow || error}
  <div class="fixed inset-0 z-[100] grid place-items-center bg-black/45 p-4" role="presentation">
    <section class="w-full max-w-md rounded-2xl border border-hairline bg-panel p-5 shadow-2xl" role="dialog" aria-modal="true" aria-labelledby="verification-title">
      <header class="mb-4 flex items-start justify-between gap-3">
        <div>
          <p class="text-xs font-semibold uppercase tracking-wider text-muted">Device security</p>
          <h2 id="verification-title" class="text-lg font-semibold">Verification request</h2>
        </div>
        <button class="rounded-lg p-2 text-muted transition hover:bg-panel-alt hover:text-primary" aria-label="Close" onclick={close}>✕</button>
      </header>

      <p class="mb-4 text-sm text-secondary">Another device is asking to verify this session.{deviceId ? ` Device: ${deviceId}` : ""}</p>
      {#if error}<p class="mb-3 rounded-lg bg-danger-soft p-3 text-sm text-danger">{error}</p>{/if}
      {#if flow}
        {#if flow.isCancelled}
          <p class="rounded-lg bg-panel-alt p-3 text-sm text-secondary">The other device cancelled this verification. Start a new request from that device to try again.</p>
        {:else if flow.isDone}
          <p class="rounded-lg bg-success-soft p-3 text-sm text-success-700-300">Devices verified successfully.</p>
        {:else if flow.canAcceptRequest}
          <button class="btn preset-filled-primary w-full justify-center" onclick={() => act("accept")} disabled={pending}>Accept request</button>
        {:else if flow.canAcceptSas}
          <div class="mb-4 grid grid-cols-4 gap-2 text-center text-2xl">
            {#each flow.emojis as emoji (`${emoji.symbol}-${emoji.description}`)}<span title={emoji.description}>{emoji.symbol}</span>{/each}
          </div>
          {#if flow.decimals}<p class="mb-4 text-center font-mono text-xl tracking-widest">{flow.decimals.join(" · ")}</p>{/if}
          <p class="mb-4 text-sm text-secondary">Compare these emojis or numbers with the other device. If they match, accept on both devices.</p>
          <button class="btn preset-filled-primary w-full justify-center" onclick={() => act("acceptSas")} disabled={pending}>They match · Accept</button>
        {:else if flow.canConfirmSas}
          <div class="mb-4 grid grid-cols-4 gap-2 text-center text-2xl">
            {#each flow.emojis as emoji (`${emoji.symbol}-${emoji.description}`)}<span title={emoji.description}>{emoji.symbol}</span>{/each}
          </div>
          {#if flow.decimals}<p class="mb-4 text-center font-mono text-xl tracking-widest">{flow.decimals.join(" · ")}</p>{/if}
          <p class="mb-4 text-sm text-secondary">Compare with the other device. Confirm only if the emojis or numbers match.</p>
          <button class="btn preset-filled-primary w-full justify-center" onclick={() => act("confirm")} disabled={pending}>Confirm match</button>
        {:else}
          <div class="flex items-center gap-3 rounded-lg bg-panel-alt p-3 text-sm text-secondary"><span class="size-4 animate-spin rounded-full border-2 border-current border-r-transparent"></span>Waiting for the other device to start SAS…</div>
        {/if}
      {/if}
    </section>
  </div>
{/if}
