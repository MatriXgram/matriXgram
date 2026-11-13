<script lang="ts">
  import { goto } from "$app/navigation";
  import { profileTarget, loadUserProfile } from "$lib/chats/user-profile";
  import { matrixGetChats, matrixOpenDirectChat } from "$lib/chats/api";
  import { shellChats, shellCurrentUserId, shellSelectedRoomId, shellSelectedRootSpaceId } from "$lib/chats/shell";
  import type { MatrixUserProfileResponse } from "$lib/chats/types";

  let dialog = $state<HTMLDialogElement | null>(null);
  let profile = $state<MatrixUserProfileResponse | null>(null);
  let loading = $state(false);
  let starting = $state(false);
  let error = $state("");
  let retry = $state(0);
  const name = $derived(profile?.displayName || $profileTarget?.displayName || $profileTarget?.userId.split(":")[0].replace(/^@/, "") || "Profile");
  const avatar = $derived(profile?.avatarUrl ?? $profileTarget?.avatarUrl);
  const server = $derived(profile?.serverName || $profileTarget?.userId.slice($profileTarget.userId.indexOf(":") + 1));

  $effect(() => {
    const target = $profileTarget;
    void retry;
    profile = null;
    error = "";
    if (!target) { dialog?.close(); return; }
    if (dialog && !dialog.open) dialog.showModal();
    let cancelled = false;
    loading = true;
    void loadUserProfile(target.roomId, target.userId).then((value) => {
      if (!cancelled) profile = value;
    }).catch((reason) => {
      if (!cancelled) error = reason instanceof Error ? reason.message : "Failed to load profile";
    }).finally(() => { if (!cancelled) loading = false; });
    return () => { cancelled = true; };
  });

  async function startChat() {
    const target = $profileTarget;
    if (!target || starting) return;
    starting = true;
    error = "";
    try {
      const { roomId } = await matrixOpenDirectChat(target.userId);
      const rooms = await matrixGetChats();
      shellChats.set(rooms);
      shellSelectedRootSpaceId.set("");
      shellSelectedRoomId.set(roomId);
      profileTarget.set(null);
      await goto(`/chats?${new URLSearchParams({ roomId })}`);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Failed to start chat";
    } finally { starting = false; }
  }
</script>

<dialog bind:this={dialog} onclose={() => profileTarget.set(null)}
  onclick={(event) => { if (event.target === dialog && !starting) profileTarget.set(null); }}
  onkeydown={(event) => { if (event.key === "Escape" && starting) event.preventDefault(); }}
  class="m-auto w-[360px] max-w-[calc(100vw-32px)] max-h-[85dvh] overflow-y-auto rounded-2xl border border-hairline bg-panel p-5 text-primary shadow-xl backdrop:bg-black/50">
  <header class="mb-4 flex items-center justify-between">
    <h2 class="text-base font-semibold">User profile</h2>
    <button type="button" class="btn btn-sm" aria-label="Close profile" disabled={starting} onclick={() => profileTarget.set(null)}>✕</button>
  </header>
  {#if $profileTarget}
    <div class="mx-auto mb-3 grid size-20 place-items-center overflow-hidden rounded-full bg-accent-soft text-2xl text-accent">
      {#if avatar}<img src={avatar} alt={name} class="size-full object-cover" />{:else}{name.charAt(0).toUpperCase()}{/if}
    </div>
    <h3 class="mb-5 break-words text-center text-lg font-semibold">{name}</h3>
    <dl class="space-y-3 text-sm">
      <div><dt class="text-xs text-secondary">Nickname</dt><dd class="break-words">{name}</dd></div>
      <div><dt class="text-xs text-secondary">Matrix ID</dt><dd class="select-text break-all">{$profileTarget.userId}</dd></div>
      <div><dt class="text-xs text-secondary">Server</dt><dd class="select-text break-all">{server}</dd></div>
    </dl>
    {#if loading}<p class="mt-3 text-xs text-secondary" role="status">Loading profile…</p>{/if}
    {#if error}<p class="mt-3 text-sm text-danger" role="alert">{error}</p>
      <button type="button" class="btn btn-sm mt-2" onclick={() => retry += 1}>Retry profile</button>{/if}
    {#if $profileTarget.userId !== $shellCurrentUserId}
      <button type="button" class="btn btn-primary mt-5 w-full" disabled={starting} onclick={startChat}>
        {starting ? "Opening chat…" : "Message"}
      </button>
    {/if}
  {/if}
</dialog>
