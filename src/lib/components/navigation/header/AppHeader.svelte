<script lang="ts">
  import { toggleMode } from "$lib/theme";

  interface Props {
    userId: string;
    avatarUrl?: string | null;
  }

  let { userId, avatarUrl = null }: Props = $props();
  let avatarFailed = $state(false);
  let previousAvatarUrl = avatarUrl;

  const displayName = $derived(userId ? userId.replace(/^@/, "").split(":")[0] : "Account");

  $effect(() => {
    const nextAvatarUrl = avatarUrl;
    if (nextAvatarUrl !== previousAvatarUrl) {
      previousAvatarUrl = nextAvatarUrl;
      avatarFailed = false;
    }
  });
</script>

<header class="account-header flex items-center justify-between gap-3">
  <div class="flex min-w-0 items-center gap-3">
    <div class="account-avatar grid size-10 shrink-0 place-items-center overflow-hidden rounded-full border border-hairline bg-accent-soft text-sm font-semibold text-accent" title={userId}>
      {#if avatarUrl && !avatarFailed}
        <img src={avatarUrl} alt="" class="h-full w-full object-cover" onerror={() => (avatarFailed = true)} />
      {:else}
        {displayName.charAt(0).toUpperCase()}
      {/if}
    </div>
    <div class="min-w-0">
      <h1 class="text-[20px] font-semibold leading-tight tracking-tight">Chats</h1>
      <p class="mt-0.5 truncate text-[11px] text-secondary" title={userId}>{userId || "Your conversations"}</p>
    </div>
  </div>
  <button type="button" class="icon-button" title="Switch theme" aria-label="Switch theme" onclick={() => toggleMode()}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" class="size-[19px]" aria-hidden="true"><path d="M20.9 13a9 9 0 0 1-9.9-9.9A9 9 0 1 0 20.9 13Z"/></svg>
  </button>
</header>
