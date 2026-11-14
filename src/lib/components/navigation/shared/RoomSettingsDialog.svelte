<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import {
		matrixRoomAddKeyword,
		matrixRoomGetKeywords,
		matrixRoomGetSettings,
		matrixRoomLeave,
		matrixRoomRemoveKeyword,
		matrixRoomRemoveTag,
		matrixRoomSetFlags,
		matrixRoomSetNotificationMode,
		matrixRoomSetTag,
} from "$lib/settings/api";
	import type {
		MatrixRoomNotificationMode,
		MatrixRoomSettingsResponse,
		MatrixRoomTag,
} from "$lib/settings/types";
	import { matrixGetRoomImage, matrixGetRoomMembers, matrixTriggerRoomUpdate } from "$lib/chats/api";
	import { shellErrorMessage, shellSelectedRoomId } from "$lib/chats/shell";
	import type { MatrixRoomMember } from "$lib/chats/types";
	import { roomImageCache } from "$lib/components/navigation/shared/image-cache.svelte";
	import MessageAvatar from "$lib/components/messaging/message/MessageAvatar.svelte";
	import { openUserProfile } from "$lib/chats/user-profile";
	import { KeywordEditor, NotificationModeSelector } from "$lib/components/settings";

	interface Props {
		open: boolean;
		roomId: string;
		roomName: string;
		isSpace: boolean;
		onClose: () => void;
	}

	let { open, roomId, roomName, isSpace, onClose }: Props = $props();

	let dialogElement: HTMLDialogElement | undefined = $state();
	let settings = $state<MatrixRoomSettingsResponse | null>(null);
	let roomAvatarUrl = $state<string | null>(null);
	let roomMembers = $state<MatrixRoomMember[]>([]);
	let loading = $state(false);
	let saving = $state(false);
	let errorMessage = $state("");
	let newTagName = $state("");
	let confirmingLeave = $state(false);
	let activeTab = $state<"profile" | "members" | "notifications" | "tags">("profile");
	let roomKeywords = $state<string[]>([]);

	const subjectLabel = $derived(isSpace ? "space" : "room");

	function formatBytes(bytes: number): string {
		const units = ["B", "KB", "MB", "GB", "TB"];
		let value = bytes;
		let unit = 0;
		while (value >= 1024 && unit < units.length - 1) {
			value /= 1024;
			unit += 1;
		}
		return `${value.toFixed(unit === 0 || value >= 10 ? 0 : 1)} ${units[unit]}`;
	}

	$effect(() => {
		if (open && dialogElement) {
			if (!dialogElement.open) {
				dialogElement.showModal();
				void loadSettings();
			}
		} else if (!open && dialogElement && dialogElement.open) {
			dialogElement.close();
		}
	});

	function resetState() {
		settings = null;
		roomAvatarUrl = null;
		roomMembers = [];
		errorMessage = "";
		newTagName = "";
		confirmingLeave = false;
		roomKeywords = [];
		activeTab = "profile";
	}

	async function loadSettings() {
		resetState();
		loading = true;

		try {
			const [nextSettings, nextKeywords, nextMembers, nextAvatar] = await Promise.all([
				matrixRoomGetSettings({ roomId }),
				matrixRoomGetKeywords({ roomId }).then((response) => response.keywords),
				matrixGetRoomMembers(roomId).catch(() => []),
				roomImageCache.getOrLoad(roomId, () => matrixGetRoomImage(roomId)),
			]);
			settings = nextSettings;
			roomKeywords = nextKeywords;
			roomMembers = nextMembers;
			roomAvatarUrl = nextAvatar;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : "Failed to load settings";
		} finally {
			loading = false;
		}
	}

	function handleClose() {
		if (saving) {
			return;
		}
		resetState();
		onClose();
	}

	async function applyChange(action: () => Promise<unknown>, rollback?: () => void) {
		saving = true;
		errorMessage = "";

		try {
			await action();
			settings = await matrixRoomGetSettings({ roomId });
		} catch (error) {
			rollback?.();
			errorMessage = error instanceof Error ? error.message : "Failed to update settings";
		} finally {
			saving = false;
		}
	}

	async function setNotificationMode(mode: MatrixRoomNotificationMode | null) {
		if (!settings || saving) {
			return;
		}
		if (mode === null && !settings.notificationModeUserDefined) {
			return;
		}
		if (mode !== null && settings.notificationModeUserDefined && mode === settings.notificationMode) {
			return;
		}

		const previousMode = settings.notificationMode;
		const previousUserDefined = settings.notificationModeUserDefined;
		settings.notificationMode = mode;
		settings.notificationModeUserDefined = mode !== null;

		saving = true;
		errorMessage = "";

		try {
			await matrixRoomSetNotificationMode({ roomId, mode });
		} catch (error) {
			if (settings) {
				settings.notificationMode = previousMode;
				settings.notificationModeUserDefined = previousUserDefined;
			}
			errorMessage = error instanceof Error ? error.message : "Failed to update settings";
		} finally {
			saving = false;
		}
	}

	function setFlags(isFavourite: boolean, isLowPriority: boolean) {
		void applyChange(() => matrixRoomSetFlags({ roomId, isFavourite, isLowPriority }));
	}

	function removeTag(tag: MatrixRoomTag) {
		void applyChange(() => matrixRoomRemoveTag({ roomId, name: tag.name }));
	}

	async function addRoomKeyword(keyword: string) {
		if (saving) {
			return;
		}
		await applyChange(() =>
			matrixRoomAddKeyword({ roomId, keyword }).then((response) => {
				roomKeywords = response.keywords;
			}),
		);
	}

	async function removeRoomKeyword(keyword: string) {
		if (saving) {
			return;
		}
		await applyChange(() =>
			matrixRoomRemoveKeyword({ roomId, keyword }).then((response) => {
				roomKeywords = response.keywords;
			}),
		);
	}

	async function addTag(event: SubmitEvent) {
		event.preventDefault();
		const name = newTagName.trim();
		if (!name || saving) {
			return;
		}

		await applyChange(() => matrixRoomSetTag({ roomId, name, order: null }));
		newTagName = "";
	}

	async function leave() {
		if (!settings?.joined || saving) {
			return;
		}

		saving = true;
		errorMessage = "";

		try {
			await matrixRoomLeave({ roomId, forget: false });
			confirmingLeave = false;
			resetState();
			onClose();

			if ($shellSelectedRoomId === roomId) {
				shellSelectedRoomId.set("");
			}
			shellErrorMessage.set("");

			await matrixTriggerRoomUpdate();

			const params = new URLSearchParams(page.url.searchParams);
			params.delete("roomId");
			const search = params.toString();
			await goto(search ? `/chats?${search}` : "/chats", { replaceState: true });
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : `Failed to leave ${subjectLabel}`;
		} finally {
			saving = false;
		}
	}
</script>

<dialog
	bind:this={dialogElement}
	onclose={handleClose}
	class="backdrop:bg-surface-900/50 card preset-outlined-surface-200-800 bg-surface-100-900 m-auto rounded-xl p-4 shadow-xl min-w-[380px] max-w-[480px] w-full max-h-[85vh] overflow-y-auto"
	onmousedown={(event) => {
		if (event.target === dialogElement) handleClose();
	}}
>
	<div class="flex flex-col gap-4">
		<header class="flex items-start justify-between gap-2">
			<div class="min-w-0">
				<p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Settings</p>
				<h2 class="h5 truncate">{isSpace ? "Space" : "Room"} settings</h2>
				<p class="text-xs text-surface-700-300 truncate">{roomName}</p>
			</div>
			<button
				type="button"
				class="btn preset-tonal px-2 py-1 text-sm"
				onclick={handleClose}
				disabled={saving}
				aria-label="Close settings"
			>
				✕
			</button>
		</header>

		<nav class="card p-2 preset-outlined-surface-200-800 bg-surface-50-950">
			<ul class="flex flex-wrap gap-2">
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "profile"}
						class:preset-tonal={activeTab !== "profile"}
						onclick={() => (activeTab = "profile")}
					>
						Profile
					</button>
				</li>
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "members"}
						class:preset-tonal={activeTab !== "members"}
						onclick={() => (activeTab = "members")}
					>
						Members
					</button>
				</li>
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "notifications"}
						class:preset-tonal={activeTab !== "notifications"}
						onclick={() => (activeTab = "notifications")}
					>
						Notifications
					</button>
				</li>
				<li>
					<button
						type="button"
						class="btn text-sm"
						class:preset-filled-primary-500={activeTab === "tags"}
						class:preset-tonal={activeTab !== "tags"}
						onclick={() => (activeTab = "tags")}
					>
						Tags
					</button>
				</li>
			</ul>
		</nav>

		{#if loading}
			<p class="text-sm text-surface-700-300">Loading settings...</p>
		{:else if settings}
			{#if errorMessage}
				<p class="text-sm preset-filled-error-500 rounded p-2">{errorMessage}</p>
			{/if}

			{#if activeTab === "profile"}
				<section class="card space-y-3 p-3 preset-outlined-surface-300-700 bg-surface-50-950">
					<div class="flex items-center gap-3">
						<div class="grid size-12 shrink-0 place-items-center overflow-hidden rounded-full bg-primary-500/15 text-lg font-semibold text-primary-600-400">
							{#if roomAvatarUrl}
								<img src={roomAvatarUrl} alt="" class="size-full object-cover" onerror={() => (roomAvatarUrl = null)} />
							{:else}
								{roomName.slice(0, 1).toUpperCase() || "#"}
							{/if}
						</div>
						<div class="min-w-0">
							<p class="text-xs text-surface-700-300">Profile</p>
							<h3 class="truncate text-base font-semibold">{roomName}</h3>
						</div>
					</div>
					<div>
						<p class="text-xs text-surface-700-300">Description</p>
						<p class="whitespace-pre-wrap break-words text-sm">{settings.topic || "No description"}</p>
					</div>
				</section>
				<section class="card space-y-2 p-3 preset-outlined-surface-300-700 bg-surface-50-950">
					<h3 class="h6">Room information</h3>
					<div class="grid gap-2 text-sm">
						<div>
							<p class="text-xs text-surface-700-300">Room ID</p>
							<p class="break-all font-mono text-xs">{settings.roomId}</p>
						</div>
						{#if settings.canonicalAlias}
							<div>
								<p class="text-xs text-surface-700-300">Room address</p>
								<p class="break-all font-mono text-xs">{settings.canonicalAlias}</p>
							</div>
						{/if}
						<div>
							<p class="text-xs text-surface-700-300">Members</p>
							<p>{settings.joinedMembers.toLocaleString()}</p>
						</div>
						<div>
							<p class="text-xs text-surface-700-300">Maximum upload size (homeserver)</p>
							<p>{settings.maxUploadSize == null ? "Unknown" : formatBytes(settings.maxUploadSize)}</p>
						</div>
					</div>
					<p class="text-xs text-surface-700-300">This is the server-reported limit; a proxy may enforce a lower one.</p>
				</section>
				{#if settings.joined}
					<section class="card space-y-2 p-3 preset-outlined-error-500 bg-surface-50-950">
						<h3 class="h6 text-error-500">Danger zone</h3>
						{#if confirmingLeave}
							<p class="text-sm text-surface-700-300">Leave this {subjectLabel}? You can rejoin later.</p>
							<div class="flex gap-2">
								<button type="button" class="btn preset-tonal px-3 py-1 text-sm" onclick={() => (confirmingLeave = false)} disabled={saving}>Cancel</button>
								<button type="button" class="btn preset-filled-error-500 px-3 py-1 text-sm" onclick={leave} disabled={saving}>{saving ? "Leaving..." : `Leave ${subjectLabel}`}</button>
							</div>
						{:else}
							<button type="button" class="btn preset-outlined-error-500 px-3 py-1 text-sm self-start" onclick={() => (confirmingLeave = true)} disabled={saving}>Leave {subjectLabel}</button>
						{/if}
					</section>
				{/if}
			{/if}

			{#if activeTab === "members"}
				<section class="card space-y-2 p-3 preset-outlined-surface-300-700 bg-surface-50-950">
					<div class="flex items-center justify-between gap-2">
						<h3 class="h6">Members</h3>
						<span class="text-xs text-surface-700-300">{settings.joinedMembers.toLocaleString()}</span>
					</div>
					{#if roomMembers.length}
						<ul class="max-h-96 divide-y divide-surface-200-800 overflow-y-auto">
							{#each roomMembers as member (member.userId)}
								<li class="flex min-w-0 items-center gap-2 py-2">
									<MessageAvatar
									{roomId}
									sender={member.userId}
									onOpenProfile={(avatarUrl) => openUserProfile({ roomId, userId: member.userId, displayName: member.displayName, avatarUrl })}
								/>
									<button type="button" class="min-w-0 flex-1 cursor-pointer text-left" onclick={() => openUserProfile({ roomId, userId: member.userId, displayName: member.displayName })}>
										<p class="truncate text-sm">{member.displayName}</p>
										<p class="truncate font-mono text-xs text-surface-700-300">{member.userId}</p>
									</button>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="text-sm text-surface-700-300">Member list unavailable.</p>
					{/if}
				</section>
			{/if}

			{#if activeTab === "notifications"}
			<div class="flex flex-col gap-3">
				<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
					<h3 class="h6">Mode</h3>
					<NotificationModeSelector
						name="room-notification-mode"
						value={settings.notificationMode}
						includeDefault={true}
						isDefault={!settings.notificationModeUserDefined}
						disabled={!settings.joined}
						onchange={setNotificationMode}
					/>
				</section>

				<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
					<div>
						<h3 class="h6">Keywords</h3>
						<p class="text-xs text-surface-700-300">
							Notify for messages in this {subjectLabel} containing a keyword, even when the
							notification mode is "Mentions and keywords only" or "Mute".
						</p>
					</div>
					<KeywordEditor
						keywords={roomKeywords}
						disabled={saving}
						emptyMessage={`No keywords for this ${subjectLabel} yet.`}
						onadd={addRoomKeyword}
						onremove={removeRoomKeyword}
					/>
				</section>
			</div>
			{/if}

			{#if activeTab === "tags"}
			<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
				<h3 class="h6">Tags</h3>
				<div class="flex flex-col gap-1">
					<label class="flex items-center gap-2 rounded p-1.5 text-sm cursor-pointer hover:bg-surface-200-800">
						<input
							type="checkbox"
							checked={settings.isFavourite}
							onchange={(event) =>
								setFlags(event.currentTarget.checked, settings?.isLowPriority ?? false)}
							disabled={saving}
						/>
						<span>Favourite</span>
					</label>
					<label class="flex items-center gap-2 rounded p-1.5 text-sm cursor-pointer hover:bg-surface-200-800">
						<input
							type="checkbox"
							checked={settings.isLowPriority}
							onchange={(event) =>
								setFlags(settings?.isFavourite ?? false, event.currentTarget.checked)}
							disabled={saving}
						/>
						<span>Low priority</span>
					</label>
				</div>

				{#if settings.tags.filter((tag) => tag.name !== "m.favourite" && tag.name !== "m.lowpriority").length > 0}
					<ul class="flex flex-wrap gap-1">
						{#each settings.tags.filter((tag) => tag.name !== "m.favourite" && tag.name !== "m.lowpriority") as tag (tag.name)}
							<li class="flex items-center gap-1 rounded-full bg-surface-200-800 px-2 py-1 text-xs">
								<span>{tag.name}</span>
								<button
									type="button"
									class="hover:text-error-500"
									aria-label={`Remove tag ${tag.name}`}
									onclick={() => removeTag(tag)}
									disabled={saving}
								>
									✕
								</button>
							</li>
						{/each}
					</ul>
				{/if}

				<form onsubmit={addTag} class="flex gap-2">
					<input
						type="text"
						placeholder="u.tag-name"
						bind:value={newTagName}
						class="input preset-outlined-surface-200-800 bg-surface-50-950 px-2 py-1 text-sm flex-1 min-w-0"
						disabled={saving}
						aria-label="New tag name"
					/>
					<button
						type="submit"
						class="btn preset-tonal px-3 py-1 text-sm"
						disabled={saving || !newTagName.trim()}
					>
						Add
					</button>
				</form>
			</section>
			{/if}

		{:else if errorMessage}
			<p class="text-sm preset-filled-error-500 rounded p-2">{errorMessage}</p>
		{/if}
	</div>
</dialog>
