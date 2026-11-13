<script lang="ts">
        import { page } from "$app/state";
        import { shellSelectedRoomId, shellSelectedRootSpaceId } from "$lib/chats/shell";

        const { children } = $props();

        const settingsNavItems = [
            { href: "/settings", label: "General" },
            { href: "/settings/notifications", label: "Notifications" },
            { href: "/settings/profile", label: "Profile" },
            { href: "/settings/account", label: "Account" },
            { href: "/settings/security", label: "Security" },
        ] as const;

        function isActivePath(href: string): boolean {
            return page.url.pathname === href;
        }
</script>

<section class="settings-layout flex h-full min-h-0 flex-col gap-4 overflow-hidden bg-panel-alt p-4">
    <nav class="settings-nav card shrink-0 p-2 preset-outlined-surface-200-800 bg-surface-50-950" aria-label="Settings navigation">
        <a
            class="btn btn-sm mb-2"
            href={`/chats?${new URLSearchParams({ roomId: $shellSelectedRoomId, rootSpaceId: $shellSelectedRootSpaceId })}`}
        >← Back to chat</a>
        <ul class="flex flex-wrap gap-2">
            {#each settingsNavItems as item (item.href)}
                <li>
                    <a
                        class="btn text-sm"
                        class:preset-filled-primary-500={isActivePath(item.href)}
                        class:preset-tonal={!isActivePath(item.href)}
                        href={item.href}
                    >
                        {item.label}
                    </a>
                </li>
            {/each}
        </ul>
    </nav>

    <div class="settings-content min-h-0 flex-1 overflow-y-auto overscroll-contain" role="region" aria-label="Settings content">
        {@render children()}
    </div>
</section>