<script lang="ts">
  import { toggleMode } from "$lib/theme";

  interface Props {
    userId: string;
  }

  let { userId }: Props = $props();

  const displayName = $derived(userId ? userId.replace(/^@/, "").split(":")[0] : "Account");
</script>

<header class="flex items-center justify-between gap-2 px-2 py-1">
  <div class="flex min-w-0 items-center gap-2">
    <div
      class="grid size-9 shrink-0 place-items-center rounded-full bg-accent-soft text-sm font-semibold text-accent"
      title={userId}
    >
      {displayName.charAt(0).toUpperCase()}
    </div>
    <div class="min-w-0">
      <p class="truncate text-sm font-semibold">{displayName}</p>
      <p class="truncate text-[11px] text-muted">{userId}</p>
    </div>  </div>

  <button
    type="button"
    class="btn btn-sm"
    title="Switch theme"
    aria-label="Switch theme"
    onclick={() => toggleMode()}
  >
    ◐
  </button>
</header>
