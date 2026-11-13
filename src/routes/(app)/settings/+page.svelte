<script lang="ts">
	import { onMount } from "svelte";
	import { get } from "svelte/store";

	import { matrixTriggerRoomUpdate } from "$lib/chats/api";
	import {
		logout as logoutSession,
		refreshRecoveryState,
		shellErrorMessage,
		shellRecoveryState,
		shellRefreshing,
		shellSelectedRoomId,
	} from "$lib/chats/shell";
	import ThemeSettings from "$lib/components/settings/ThemeSettings.svelte";
	import { recoveryStateLabel } from "$lib/components/verification/helpers";
	import { watchSystemTheme } from "$lib/theme";

	let loggingOut = $state(false);
	let settingsMessage = $state("");

	onMount(() => watchSystemTheme());

	async function refreshRooms() {
		if (get(shellRefreshing)) {
			return;
		}

		shellRefreshing.set(true);
		settingsMessage = "";
		shellErrorMessage.set("");

		try {
			await matrixTriggerRoomUpdate({
				selectedRoomId: get(shellSelectedRoomId) || null,
				includeSelectedMessages: false,
			});

			await refreshRecoveryState();

			settingsMessage = "Refresh requested.";
		} catch (error) {
			shellErrorMessage.set(error instanceof Error ? error.message : "Failed to trigger room refresh");
		} finally {
			shellRefreshing.set(false);
		}
	}

	async function logout() {
		loggingOut = true;
		settingsMessage = "";
		shellErrorMessage.set("");

		try {
			const error = await logoutSession();
			if (error) {
				shellErrorMessage.set(error);
			}
		} finally {
			loggingOut = false;
		}
	}
</script>

<section class="space-y-4">
	<header>
		<p class="text-xs font-bold uppercase tracking-[0.2em] text-accent">Settings</p>
		<h2 class="h5">Account and Security</h2>
		<p class="text-xs text-secondary">{recoveryStateLabel($shellRecoveryState)}</p>
	</header>

	{#if $shellErrorMessage}
		<p class="card p-2 text-sm preset-filled-error-500">{$shellErrorMessage}</p>
	{/if}

	{#if settingsMessage}
		<p class="text-xs text-secondary">{settingsMessage}</p>
	{/if}

	<ThemeSettings />

	<section class="card space-y-2 border border-hairline bg-panel p-3">
		<h3 class="h6">Sync</h3>
		<p class="text-xs text-secondary">Request a room refresh from the server.</p>
		<button
			type="button"
			class="btn preset-tonal"
			onclick={refreshRooms}
			disabled={$shellRefreshing}
		>
			{#if $shellRefreshing}Refreshing...{:else}Refresh{/if}
		</button>
	</section>

	<section class="card space-y-2 border border-hairline bg-panel p-3">
		<h3 class="h6">Session</h3>
		<p class="text-xs text-secondary">Sign out from the current account on this device.</p>
		<button
			type="button"
			class="btn preset-filled-error-500"
			onclick={logout}
			disabled={loggingOut}
		>
			{#if loggingOut}Logging out...{:else}Logout{/if}
		</button>
	</section>
</section>
