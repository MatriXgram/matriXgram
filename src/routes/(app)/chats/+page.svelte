<script lang="ts">
  import { page } from "$app/state";
  import CommunityExplorer from "$lib/components/navigation/rooms/CommunityExplorer.svelte";
import {
  matrixGetRoomReadState,
  matrixMarkRoomRead,
  matrixGetReadContext,
  matrixGetNewerMessages,
  matrixGetUploadLimit,
  matrixPrefetchPinnedMessageContexts,
} from "$lib/chats/api";
  import { firstUnreadIndex, isUnreadMessage } from "$lib/chats/read-position";
import { formatDraft } from "$lib/components/messaging/composer/draftFormatting";
import type { ComposerMention } from "$lib/components/messaging/composer";
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open } from "@tauri-apps/plugin-dialog";

  import {
    matrixCancelMediaTranscode,
    matrixGetFileSize,
    matrixRemovePastedFile,
    matrixSendChatMessage,
    matrixSendMediaFile,
    matrixStreamChatMessages,
    matrixToggleReaction,
    matrixRedactMessage,
  } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import { consumeReactionTarget, reactionNotices } from "$lib/chats/reaction-notifications";
  import { EVENT_MEDIA_TRANSCODE_PROGRESS } from "$lib/events";
  import {
    shellChats,
    shellCurrentUserId,
    shellPickerCustomEmoji,
    shellSelectedRoomId,
  } from "$lib/chats/shell";
  import type {
    MatrixChatMessage,
    MatrixChatMessageStreamEvent,
    MatrixMediaTranscodeProgressEvent,
    MatrixSelectedRoomMessagesEvent,
    MatrixSendChatMessageRequest,
    MatrixMessageLoadKind,
    MatrixReactionSummary,
  } from "$lib/chats/types";
  import {
    normalizeReactionKey,
  } from "$lib/emoji/picker";
  import { RoomSettingsDialog } from "$lib/components/navigation";
  import { MessageComposer } from "$lib/components/messaging/composer";
  import { EmojiPickerPopover } from "$lib/components/messaging/composer-emoji";
  import ForwardRoomPicker from "$lib/components/messaging/forward/ForwardRoomPicker.svelte";
  import { MessageTimeline } from "$lib/components/messaging/timeline";
  import type { TimelineMessage } from "$lib/components/messaging/shared";
import { replyFallback, stripMxReplyBlock } from "$lib/components/messaging/shared";

  let loadingMessages = $state(false);
  let errorMessage = $state("");
  let readPositionError = $state("");
  let composerErrorMessage = $state("");
  let messageDraft = $state("");
  let sendingMessage = $state(false);
  let replyToMessage = $state<TimelineMessage | null>(null);
  let composerFocusNonce = $state(0);
  let roomSettingsDialogRoomId = $state<string | null>(null);
  let pendingMedia = $state<{
    filePath: string;
    fileName: string;
    fileSize: number | null;
    maxUploadSize: number | null;
    checkingLimit: boolean;
    limitError: string;
    messageType: "m.image" | "m.video" | "m.file";
    compressMedia: boolean;
  } | null>(null);
  let mediaCaption = $state("");
  let mediaCaptionInput = $state<HTMLInputElement | HTMLTextAreaElement | null>(null);
  let mediaEmojiPickerOpen = $state(false);
  let sendingMedia = $state(false);
  let mediaErrorMessage = $state("");
  let activeMediaFilePath = $state("");
  let mediaTranscodeProgress = $state<MatrixMediaTranscodeProgressEvent | null>(null);
  let composerDockElement = $state<HTMLDivElement | null>(null);
  const mediaDraftsByLocalId = new Map<string, { filePath: string; compressMedia: boolean; caption: string; roomId: string }>();
  let forwardingMessage = $state<TimelineMessage | null>(null);
  let forwardError = $state("");
  let forwarding = $state(false);
  let prefetchedPinnedRoomsKey = "";

  let messages = $state<TimelineMessage[]>([]);
  let nextFrom = $state<string | null>(null);
  let timelineElement = $state<HTMLElement | null>(null);
  let activeStreamId = $state("");
  let activeLoadKind = $state<MatrixMessageLoadKind | null>(null);
  let streamMessageCount = $state(0);
  let roomUpdatesReady = $state(false);
  let roomUpdatesError = $state("");
  let retryRoomUpdates = () => {};
  let initialLoadGeneration = 0;

  let unreadBoundaryId = $state<string | null>(null);
  let unreadRemaining = $state(0);
  let unreadTotal = 0;
  let initialLatestTimestamp = 0;
  let readMarkerId: string | null = null;
  let readPositionReady = $state(false);
  let readStateConfirmed = false;
  let initialPositionPending = $state(true);
  let positioningInitial = false;
  let nextTo = $state<string | null>(null);
  let loadingNewer = $state(false);
  let readingContext = false;
  let jumpRequested = false;
  let highestReadTimestamp = 0;
  const consumedUnread = new Set<string>();
  const newUnread = new Set<string>();
  const unreadAttentionMessages = $derived.by(() => {
    const userId = $shellCurrentUserId;
    if (!userId || unreadRemaining <= 0) return [] as TimelineMessage[];
    const startIndex = unreadBoundaryId ? messages.findIndex((message) => message.eventId === unreadBoundaryId) : -1;
    const unreadMessages = messages.filter((message, index) => {
      if (message.sender === userId || !message.eventId) return false;
      if (consumedUnread.has(message.eventId)) return false;
      return newUnread.has(message.eventId) || (startIndex >= 0 && index >= startIndex);
    });
    return unreadMessages.filter((message) => {
      if (message.mentionsMe) return true;
      if (!message.inReplyToEventId) return false;
      const target = messages.find((candidate) => candidate.eventId === message.inReplyToEventId);
      return target?.sender === userId || replyFallback(message).sender === userId;
    });
  });
  const hasUnreadAttention = $derived(unreadAttentionMessages.length > 0);
  let attentionJumpRequest = $state<{ eventId: string; nonce: number } | null>(null);
  let attentionJumpNonce = 0;
  let reactionJumpRequest = $state<{ roomId: string; eventId: string; nonce: number } | null>(null);
  let reactionJumpNonce = 0;
  const unreadReactionTargets = $derived([...new Set($reactionNotices
    .filter((notice) => notice.roomId === $shellSelectedRoomId)
    .map((notice) => notice.targetEventId))]);
  let readTimer: ReturnType<typeof setTimeout>;
  let readRequestInFlight = false;

  const seenEventIds = new Set<string>();
  const reactionStateOverrides = new Map<
    string,
    Map<string, { added: boolean; timeout: ReturnType<typeof setTimeout>; token: symbol }>
  >();

  const AUTO_LOAD_TOP_THRESHOLD_PX = 600;
  const REPLY_JUMP_OLDER_EDGE_MESSAGES = 10;
  const NEAR_BOTTOM_PX = 48;
  const PROGRAMMATIC_SCROLL_SETTLE_MS = 800;
  // The room worker holds the per-homeserver sync lock for as long as the
  // server keeps a long poll open, and a send queues behind it, so a slow
  // homeserver can spend this long before the first page lands.
  const MESSAGE_LOAD_TIMEOUT_MS = 30000;

  let pendingPinToBottomRoomId = "";
  let pendingRoomMessages: MatrixSelectedRoomMessagesEvent | null = null;
  let pendingOlderMessages: MatrixChatMessage[] = [];
  type RoomViewportAnchor = { eventId: string; offset: number };
  let pendingRoomViewportAnchor: RoomViewportAnchor | null = null;
  const roomViewportAnchors = new Map<string, RoomViewportAnchor>();

  let previousSelectedRoomId = $state("");
  let lastScrollTop = 0;
  let restoringTimelinePosition = false;
  let isScrolledToBottom = $state(true);
  let programmaticScrollUntil = 0;
  let viewportAnchorSaveTimer: ReturnType<typeof setTimeout>;

  onMount(() => {
    let unlisten = () => {};
    let unlistenTranscode = () => {};
    let unlistenDrop = () => {};
    let disposed = false;
    let subscribing = false;

    async function subscribeRoomEvents() {
      if (subscribing || roomUpdatesReady) return;
      subscribing = true;
      roomUpdatesError = "";
      try {
        const stop = await subscribeToRoomUpdates({
          onRoomAdded: () => {},
          onRoomUpdated: () => {},
          onRoomRemoved: () => {},
          onSelectedRoomMessages: applySelectedRoomMessages,
          onChatMessagesStream: applyChatMessageStream,
        });
        if (disposed) { stop(); return; }
        unlisten = stop;
        roomUpdatesReady = true;
      } catch (error) {
        if (!disposed) roomUpdatesError = error instanceof Error ? error.message : "Failed to subscribe to room updates";
      } finally {
        subscribing = false;
      }
    }
    retryRoomUpdates = () => { void subscribeRoomEvents(); };
    void subscribeRoomEvents();

    void (async () => {
      const stopTranscode = await listen<MatrixMediaTranscodeProgressEvent>(
        EVENT_MEDIA_TRANSCODE_PROGRESS,
        (event) => {
          if (event.payload.filePath !== activeMediaFilePath) {
            return;
          }

          mediaTranscodeProgress = event.payload;
        },
      );
      if (disposed) { stopTranscode(); return; }
      unlistenTranscode = stopTranscode;
      const stopDrop = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type !== "drop" || !event.payload.paths.length || !composerDockElement) return;
        const scale = window.devicePixelRatio || 1;
        const x = event.payload.position.x / scale;
        const y = event.payload.position.y / scale;
        const bounds = composerDockElement.getBoundingClientRect();
        if (x < bounds.left || x > bounds.right || y < bounds.top || y > bounds.bottom) return;
        createPendingMedia(event.payload.paths[0]);
      });
      if (disposed) { stopDrop(); return; }
      unlistenDrop = stopDrop;
    })();

    return () => {
      disposed = true;
      initialLoadGeneration++;
      clearTimeout(readTimer);
      clearTimeout(viewportAnchorSaveTimer);
      unlisten();
      unlistenTranscode();
      unlistenDrop();
    };
  });

  $effect(() => {
    const roomIds = $shellChats
      .filter((room) => room.joined && room.kind === "room" && !room.roomId.startsWith("virtual:"))
      .map((room) => room.roomId);
    const key = [...roomIds].sort().join("\n");
    if (!key) {
      prefetchedPinnedRoomsKey = "";
      return;
    }
    if (key === prefetchedPinnedRoomsKey) return;
    prefetchedPinnedRoomsKey = key;
    void matrixPrefetchPinnedMessageContexts(roomIds);
  });

  $effect(() => {
    const selectedRoomId = $shellSelectedRoomId;
    if (!roomUpdatesReady) return;

    if (!selectedRoomId) {
      if (previousSelectedRoomId) rememberRoomViewport(previousSelectedRoomId);
      previousSelectedRoomId = "";
      loadingMessages = false;
      resetRoomState();
      return;
    }

    if (selectedRoomId === previousSelectedRoomId) {
      return;
    }

    if (previousSelectedRoomId) rememberRoomViewport(previousSelectedRoomId);
    previousSelectedRoomId = selectedRoomId;
    resetRoomState();
    pendingRoomViewportAnchor = selectedRoomId ? readRoomViewportAnchor(selectedRoomId) : null;

    void loadMessages(selectedRoomId);
  });

  function roomViewportStorageKey(roomId: string): string {
    return `matrixgram:timeline-anchor:${$shellCurrentUserId}:${roomId}`;
  }

  function readRoomViewportAnchor(roomId: string): RoomViewportAnchor | null {
    const inMemory = roomViewportAnchors.get(roomId);
    if (inMemory) return inMemory;
    try {
      const stored = localStorage.getItem(roomViewportStorageKey(roomId));
      if (!stored) return null;
      const anchor = JSON.parse(stored) as Partial<RoomViewportAnchor>;
      if (typeof anchor.eventId !== "string" || typeof anchor.offset !== "number" || !Number.isFinite(anchor.offset)) return null;
      const result = { eventId: anchor.eventId, offset: anchor.offset };
      roomViewportAnchors.set(roomId, result);
      return result;
    } catch {
      return null;
    }
  }

  function rememberRoomViewport(roomId: string) {
    const element = timelineElement;
    if (!element) return;
    const viewportTop = element.getBoundingClientRect().top;
    const row = Array.from(element.querySelectorAll<HTMLElement>("[data-message-event-id]"))
      .find((candidate) => candidate.getBoundingClientRect().bottom > viewportTop);
    const eventId = row?.dataset.messageEventId;
    if (!row || !eventId) return;
    const anchor = { eventId, offset: row.getBoundingClientRect().top - viewportTop };
    roomViewportAnchors.set(roomId, anchor);
    try {
      localStorage.setItem(roomViewportStorageKey(roomId), JSON.stringify(anchor));
    } catch {
      // The in-memory anchor still works when persistent browser storage is unavailable.
    }
  }

  function distanceToBottom(element: HTMLElement): number {
    return Math.max(0, element.scrollHeight - element.clientHeight - element.scrollTop);
  }

  function beginProgrammaticScroll() {
    programmaticScrollUntil = performance.now() + PROGRAMMATIC_SCROLL_SETTLE_MS;
  }

  function handleTimelineScroll() {
    const selectedRoomId = $shellSelectedRoomId;
    if (!timelineElement) {
      return;
    }

    const scrollTop = timelineElement.scrollTop;
    const scrollingUp = scrollTop < lastScrollTop;
    lastScrollTop = scrollTop;
    isScrolledToBottom = distanceToBottom(timelineElement) <= NEAR_BOTTOM_PX;
    if (selectedRoomId) {
      clearTimeout(viewportAnchorSaveTimer);
      viewportAnchorSaveTimer = setTimeout(() => rememberRoomViewport(selectedRoomId), 250);
    }

    if (!selectedRoomId || performance.now() < programmaticScrollUntil) {
      return;
    }

    if (scrollingUp) prefetchOlderMessages();
    if (nextTo && distanceToBottom(timelineElement) < timelineElement.clientHeight * 1.5) void loadNewer();
    scheduleReadVisible();
  }

  function prefetchOlderMessages() {
    const element = timelineElement;
    if (!element || !$shellSelectedRoomId || loadingMessages || restoringTimelinePosition || !nextFrom ||
        performance.now() < programmaticScrollUntil) return;

    // Start fetching while there is still history above the viewport to read.
    const threshold = Math.max(AUTO_LOAD_TOP_THRESHOLD_PX, element.clientHeight * 1.5);
    if (element.scrollTop <= threshold) void loadOlder();
  }

  $effect(() => {
    const element = timelineElement;
    if (!element) return;
    // A short page may not scroll at all. Upward wheel intent still fetches history.
    const onWheel = (event: WheelEvent) => {
      if (event.deltaY < 0) prefetchOlderMessages();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "PageUp" || event.key === "ArrowUp" || event.key === "Home") {
        prefetchOlderMessages();
      }
    };
    element.addEventListener("wheel", onWheel, { passive: true });
    element.addEventListener("keydown", onKeyDown);
    return () => {
      element.removeEventListener("wheel", onWheel);
      element.removeEventListener("keydown", onKeyDown);
    };
  });

  function createStreamId(): string {
    if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
      return crypto.randomUUID();
    }

    return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  // `force` holds the viewport on the newest message regardless of where it
  // currently is; without it, only a timeline already parked at the bottom
  // stays there, so history stays put while it is being read.
  function queuePinTimelineToBottom(roomId: string, options: { force?: boolean } = {}) {
    if (!options.force && (!readPositionReady || initialPositionPending || (readingContext && nextTo))) return;
    if (!options.force && !isScrolledToBottom) {
      return;
    }

    if (pendingPinToBottomRoomId === roomId) {
      return;
    }

    pendingPinToBottomRoomId = roomId;

    void (async () => {
      await tick();
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

      if (!timelineElement || $shellSelectedRoomId !== roomId) {
        pendingPinToBottomRoomId = "";
        return;
      }

      beginProgrammaticScroll();
      const maxScrollTop = Math.max(0, timelineElement.scrollHeight - timelineElement.clientHeight);
      timelineElement.scrollTop = maxScrollTop;
      lastScrollTop = timelineElement.scrollTop;
      isScrolledToBottom = true;
      rememberRoomViewport(roomId);

      pendingPinToBottomRoomId = "";
    })();
  }

  function isDuplicateMessage(message: MatrixChatMessage): boolean {
    if (!message.eventId) {
      return false;
    }

    if (seenEventIds.has(message.eventId)) {
      return true;
    }

    seenEventIds.add(message.eventId);
    return false;
  }

  function tryReplaceOptimisticWithRemote(message: MatrixChatMessage): boolean {
    if (message.timestamp == null) {
      return false;
    }

    const incomingTimestamp = message.timestamp;

    const optimisticIndex = messages.findIndex((candidate) => {
      if (candidate.sendState !== "sending" && candidate.sendState !== "failed") {
        return false;
      }

      if (candidate.body !== message.body || candidate.sender !== message.sender) {
        return false;
      }

      if (!candidate.timestamp) {
        return false;
      }

      return Math.abs(candidate.timestamp - incomingTimestamp) <= 120_000;
    });

    if (optimisticIndex < 0) {
      return false;
    }

    const updated = [...messages];
    // The timeline keys a row by `localId` before the echo arrives, so the id
    // has to survive the swap or the row is rebuilt under a different key.
    updated[optimisticIndex] = { ...message, localId: messages[optimisticIndex].localId };
    messages = updated;
    return true;
  }

  function preserveReadingPosition(update: () => void) {
    const element = timelineElement;
    const roomId = $shellSelectedRoomId;
    if (!element) { update(); return; }

    const viewportTop = element.getBoundingClientRect().top;
    const anchor = Array.from(element.querySelectorAll<HTMLElement>("[data-message-event-id]"))
      .find((row) => row.getBoundingClientRect().bottom > viewportTop);
    const anchorTop = anchor?.getBoundingClientRect().top;
    const oldHeight = element.scrollHeight;
    const oldScrollTop = element.scrollTop;
    restoringTimelinePosition = true;
    update();

    void tick().then(() => {
      if (timelineElement !== element || $shellSelectedRoomId !== roomId) return;
      if (anchor?.isConnected && anchorTop !== undefined) {
        // Measure after Svelte and browser scroll anchoring: compensate only
        // for the displacement that remains, avoiding a double correction.
        element.scrollTop += anchor.getBoundingClientRect().top - anchorTop;
      } else {
        element.scrollTop = oldScrollTop + element.scrollHeight - oldHeight;
      }
      lastScrollTop = element.scrollTop;
      isScrolledToBottom = distanceToBottom(element) <= NEAR_BOTTOM_PX;
      requestAnimationFrame(() => {
        if ($shellSelectedRoomId === roomId) restoringTimelinePosition = false;
      });
    });
  }

  function applyChatMessageStream(payload: MatrixChatMessageStreamEvent) {
    if (payload.roomId !== $shellSelectedRoomId) {
      return;
    }

    if (!activeStreamId || payload.streamId !== activeStreamId || payload.loadKind !== activeLoadKind) {
      return;
    }

    if (payload.done) {
      if (!(readingContext && nextTo && payload.loadKind === "initial")) nextFrom = payload.nextFrom ?? null;
      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      const streamError = payload.error;
      if (streamError) errorMessage = streamError;
      else errorMessage = "";

      // A room update that arrived while the timeline was streaming is
      // applied now that the stream is done.
      if (payload.loadKind === "older") flushPendingOlderMessages();
      flushPendingRoomMessages();

      // A fresh room always lands on the newest message, wherever the previous
      // room was scrolled to. Empty rooms have no position to calculate.
      if (payload.loadKind === "initial") {
        if (!messages.length) initialPositionPending = false;
        queuePinTimelineToBottom(payload.roomId);
      }
      return;
    }

    if (payload.messages) {
      if (payload.loadKind === "older") {
        for (const message of payload.messages) {
          if (!isDuplicateMessage(message)) pendingOlderMessages.push(message);
        }
        streamMessageCount = payload.sequence + payload.messages.length;
        return;
      }
      if (readingContext && nextTo && payload.loadKind === "initial") {
        pendingRoomMessages = { roomId: payload.roomId, nextFrom: null, messages: payload.messages };
        return;
      }
      applySelectedRoomMessagesNow({ roomId: payload.roomId, nextFrom: null, messages: payload.messages });
      streamMessageCount = payload.sequence + payload.messages.length;
      return;
    }

    if (!payload.message || isDuplicateMessage(payload.message)) {
      return;
    }

    if (tryReplaceOptimisticWithRemote(payload.message)) {
      if (payload.message.eventId) {
        seenEventIds.add(payload.message.eventId);
      }
      return;
    }

    streamMessageCount = payload.sequence + 1;

    if (payload.loadKind === "older") {
      pendingOlderMessages.push(payload.message);
    } else {
      // Initial streaming emits oldest->newest so appending keeps timeline ascending.
      messages = [...messages, payload.message];
    }
  }

  function applySelectedRoomMessages(payload: MatrixSelectedRoomMessagesEvent) {
    if (payload.roomId !== $shellSelectedRoomId) {
      return;
    }

    if (readingContext && nextTo) {
      trackNewUnread(payload.messages);
      pendingRoomMessages = payload;
      return;
    }
    if (loadingMessages && activeLoadKind !== "initial") {
      // Older history is still being streamed. The payload is kept and applied
      // when the stream finishes, otherwise the messages it carries would be
      // dropped and the room would keep showing an older page.
      pendingRoomMessages = payload;
      return;
    }

    applySelectedRoomMessagesNow(payload);
  }

  function flushPendingRoomMessages() {
    if (loadingMessages || (readingContext && nextTo) || !pendingRoomMessages) {
      return;
    }

    const payload = pendingRoomMessages;
    pendingRoomMessages = null;
    applySelectedRoomMessagesNow(payload);
  }

  function flushPendingOlderMessages() {
    if (!pendingOlderMessages.length) return;
    // The pagination stream arrives newest-to-oldest. Reverse once, prepend the
    // full page, and preserve the viewport once instead of correcting scroll
    // position for every individual event as it streams in.
    const olderMessages = pendingOlderMessages.reverse();
    pendingOlderMessages = [];
    preserveReadingPosition(() => { messages = [...olderMessages, ...messages]; });
  }

  function applySelectedRoomMessagesNow(payload: MatrixSelectedRoomMessagesEvent) {
    const indexed = payload.messages
      .map((message, index) => ({ message, index }))
      .filter(({ message }) => Boolean(message.eventId));

    if (!indexed.length) {
      return;
    }

    for (const { message } of indexed) {
      if (!message.eventId || !seenEventIds.has(message.eventId)) {
        continue;
      }

      messages = messages.map((candidate) => {
        if (candidate.eventId !== message.eventId) {
          return candidate;
        }

        const reactions = applyReactionStateOverrides(message.eventId!, message.reactions);
        return {
          ...candidate,
          ...message,
          reactions,
          imageUrl: message.imageUrl ?? candidate.imageUrl,
          thumbnailUrl: message.thumbnailUrl ?? candidate.thumbnailUrl,
          localId: candidate.localId,
          sendState: candidate.sendState,
        };
      });
    }

    const seenIndexes = indexed
      .filter(({ message }) => Boolean(message.eventId && seenEventIds.has(message.eventId)))
      .map(({ index }) => index);

    const newerCandidates: MatrixChatMessage[] = [];
    const olderCandidates: MatrixChatMessage[] = [];

    if (!seenIndexes.length) {
      for (const { message } of indexed) {
        if (!message.eventId || seenEventIds.has(message.eventId)) {
          continue;
        }
        newerCandidates.push(message);
      }
    } else {
      const firstSeenIndex = Math.min(...seenIndexes);
      const lastSeenIndex = Math.max(...seenIndexes);

      for (const { message, index } of indexed) {
        if (!message.eventId || seenEventIds.has(message.eventId)) {
          continue;
        }

        if (index < firstSeenIndex) {
          olderCandidates.push(message);
          continue;
        }

        if (index > lastSeenIndex) {
          newerCandidates.push(message);
        }
      }
    }

    const prependOlder: TimelineMessage[] = [];
    for (const message of olderCandidates) {
      if (tryReplaceOptimisticWithRemote(message)) {
        if (message.eventId) {
          seenEventIds.add(message.eventId);
        }
        continue;
      }

      if (message.eventId) {
        seenEventIds.add(message.eventId);
      }

      prependOlder.push(message);
    }

    const appendNewer: TimelineMessage[] = [];
    for (const message of newerCandidates) {
      if (tryReplaceOptimisticWithRemote(message)) {
        if (message.eventId) {
          seenEventIds.add(message.eventId);
        }
        continue;
      }

      if (message.eventId) {
        seenEventIds.add(message.eventId);
      }

      appendNewer.push(message);
    }

    if (!prependOlder.length && !appendNewer.length) {
      return;
    }

    if (prependOlder.length) {
      preserveReadingPosition(() => { messages = [...prependOlder, ...messages, ...appendNewer]; });
    } else {
      messages = [...messages, ...appendNewer];
    }
    if (appendNewer.length) {
      queuePinTimelineToBottom(payload.roomId);
    }
  }

  function buildOptimisticMessage(body: string, inReplyToEventId: string | null): TimelineMessage {
    const encryptedRoom = selectedRoomEncrypted();

    return {
      eventId: null,
      inReplyToEventId,
      sender: $shellCurrentUserId || "You",
      timestamp: Date.now(),
      body,
      formattedBody: null,
      messageType: "m.text",
      imageUrl: null,
      thumbnailUrl: null,
      customEmojis: [],
      reactions: [],
      encrypted: encryptedRoom,
      decryptionStatus: encryptedRoom ? "decrypted" : "plaintext",
      verificationStatus: "unknown",
      isDeleted: false,
      threadReplyCount: 0,
      mentionsMe: false,
      isSticker: false,
      isVoice: false,
      localId: createStreamId(),
      sendState: "sending",
    };
  }

  function buildOptimisticMediaMessage(): TimelineMessage {
    const encryptedRoom = selectedRoomEncrypted();

    if (!pendingMedia) {
      return buildOptimisticMessage("Attachment", null);
    }

    return {
      eventId: null,
      inReplyToEventId: null,
      sender: $shellCurrentUserId || "You",
      timestamp: Date.now(),
      body: mediaCaption.trim() || pendingMedia.fileName,
      formattedBody: null,
      messageType: pendingMedia.messageType,
      imageUrl:
        pendingMedia.messageType === "m.file"
          ? null
          : convertFileSrc(pendingMedia.filePath),
      thumbnailUrl: null,
      customEmojis: [],
      reactions: [],
      encrypted: encryptedRoom,
      decryptionStatus: encryptedRoom ? "decrypted" : "plaintext",
      verificationStatus: "unknown",
      isDeleted: false,
      threadReplyCount: 0,
      mentionsMe: false,
      isSticker: false,
      isVoice: false,
      localId: createStreamId(),
      sendState: "sending",
    };
  }

  function detectMediaKind(filePath: string): "m.image" | "m.video" | "m.file" {
    const extension = filePath.split(".").pop()?.toLowerCase() ?? "";

    if (["png", "jpg", "jpeg", "gif", "webp", "bmp"].includes(extension)) {
      return "m.image";
    }

    if (["mp4", "mkv", "mov", "webm", "avi"].includes(extension)) {
      return "m.video";
    }

    return "m.file";
  }

  async function validatePendingMedia(filePath: string) {
    if (pendingMedia?.filePath !== filePath) return;
    pendingMedia = { ...pendingMedia, checkingLimit: true, limitError: "" };
    try {
      const [fileSize, maxUploadSize] = await Promise.all([
        matrixGetFileSize(filePath),
        matrixGetUploadLimit(),
      ]);
      if (pendingMedia?.filePath !== filePath) return;
      pendingMedia = { ...pendingMedia, fileSize, maxUploadSize, checkingLimit: false };
      mediaErrorMessage = fileSize > maxUploadSize
        ? `This file is larger than this homeserver’s ${formatFileSize(maxUploadSize)} upload limit.`
        : "";
    } catch (error) {
      if (pendingMedia?.filePath !== filePath) return;
      pendingMedia = {
        ...pendingMedia,
        checkingLimit: false,
        limitError: error instanceof Error ? error.message : "Could not check the homeserver upload limit.",
      };
      mediaErrorMessage = pendingMedia.limitError;
    }
  }

  function createPendingMedia(filePath: string) {
    const messageType = detectMediaKind(filePath);
    pendingMedia = {
      filePath,
      fileName: filePath.split(/[\\/]/).pop() || "attachment",
      fileSize: null,
      maxUploadSize: null,
      checkingLimit: true,
      limitError: "",
      messageType,
      compressMedia: messageType !== "m.video",
    };
    mediaCaption = "";
    mediaEmojiPickerOpen = false;
    mediaErrorMessage = "";
    void validatePendingMedia(filePath);
  }

  function fileExtension(fileName: string): string {
    const dot = fileName.lastIndexOf(".");
    return dot > 0 ? fileName.slice(dot + 1).toUpperCase() : "FILE";
  }

  function fileBadge(fileName: string): { label: string; color: string } {
    const extension = fileExtension(fileName).toLowerCase();
    if (["doc", "docx", "odt", "rtf"].includes(extension)) return { label: "W", color: "bg-blue-500/15 text-blue-500" };
    if (["xls", "xlsx", "ods", "csv"].includes(extension)) return { label: "X", color: "bg-green-500/15 text-green-600" };
    if (["ppt", "pptx", "odp"].includes(extension)) return { label: "P", color: "bg-orange-500/15 text-orange-500" };
    if (extension === "pdf") return { label: "PDF", color: "bg-red-500/15 text-red-500" };
    if (["zip", "rar", "7z", "tar", "gz"].includes(extension)) return { label: "ZIP", color: "bg-amber-500/15 text-amber-600" };
    return { label: extension.slice(0, 4), color: "bg-surface-300-700 text-surface-700-300" };
  }

  function formatFileSize(bytes: number | null): string {
    if (bytes === null) return "Reading file size…";
    if (bytes < 1024) return `${bytes} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let size = bytes / 1024;
    let unit = 0;
    while (size >= 1024 && unit < units.length - 1) {
      size /= 1024;
      unit += 1;
    }
    return `${size.toFixed(size >= 10 ? 1 : 2)} ${units[unit]}`;
  }

  function insertMediaCaptionEmoji(emoji: string) {
    const input = mediaCaptionInput;
    if (!input) {
      mediaCaption += emoji;
      return;
    }
    const start = input.selectionStart ?? mediaCaption.length;
    const end = input.selectionEnd ?? start;
    mediaCaption = `${mediaCaption.slice(0, start)}${emoji}${mediaCaption.slice(end)}`;
    void tick().then(() => {
      input.focus();
      const cursor = start + emoji.length;
      input.setSelectionRange(cursor, cursor);
    });
  }

  async function chooseAttachment() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Choose a file to send",
    });

    if (typeof selected === "string" && selected.length > 0) {
      createPendingMedia(selected);
    }
  }

  function handlePendingMediaEscape(event: KeyboardEvent) {
    if (event.key !== "Escape" || !pendingMedia) {
      return;
    }

    event.preventDefault();
    void cancelPendingMedia();
  }

  function handlePendingMediaBackdropClick(event: MouseEvent) {
    if (!pendingMedia || event.currentTarget !== event.target) {
      return;
    }

    void cancelPendingMedia();
  }

  function handlePendingMediaBackdropKeydown(event: KeyboardEvent) {
    if (!pendingMedia || event.currentTarget !== event.target) {
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void cancelPendingMedia();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handlePendingMediaEscape);

    return () => {
      window.removeEventListener("keydown", handlePendingMediaEscape);
    };
  });

  async function sendPendingMedia(retryLocalId?: string) {
    const retryDraft = retryLocalId ? mediaDraftsByLocalId.get(retryLocalId) : undefined;
    const currentPendingMedia = pendingMedia;
    const roomId = retryDraft?.roomId ?? $shellSelectedRoomId;
    const mediaDraft = retryDraft ?? (currentPendingMedia && roomId ? {
      filePath: currentPendingMedia.filePath,
      compressMedia: currentPendingMedia.compressMedia,
      caption: mediaCaption.trim(),
      roomId,
    } : null);
    if (!mediaDraft || sendingMedia || !roomId) {
      return;
    }

    if (!retryDraft && currentPendingMedia?.checkingLimit) return;
    if (!retryDraft && currentPendingMedia?.limitError) {
      mediaErrorMessage = currentPendingMedia.limitError;
      return;
    }
    if (!retryDraft && currentPendingMedia && !currentPendingMedia.compressMedia && currentPendingMedia.fileSize !== null && currentPendingMedia.maxUploadSize !== null && currentPendingMedia.fileSize > currentPendingMedia.maxUploadSize) {
      mediaErrorMessage = `This file is larger than this homeserver’s ${formatFileSize(currentPendingMedia.maxUploadSize)} upload limit.`;
      return;
    }

    activeMediaFilePath = mediaDraft.filePath;
    mediaTranscodeProgress = {
      roomId,
      filePath: mediaDraft.filePath,
      stage: mediaDraft.compressMedia ? "preparing" : "uploading",
      progress: 0,
      hardwareMode: "",
    };
    const optimistic = retryLocalId ? null : buildOptimisticMediaMessage();
    const localId = retryLocalId ?? optimistic?.localId;

    if (!localId) {
      return;
    }

    if (optimistic) {
      messages = [...messages, optimistic];
      mediaDraftsByLocalId.set(localId, {
        filePath: mediaDraft.filePath,
        compressMedia: mediaDraft.compressMedia,
        caption: mediaCaption.trim(),
        roomId,
      });
      queuePinTimelineToBottom(roomId, { force: true });
    } else {
      messages = messages.map((message) => message.localId === localId
        ? { ...message, sendState: "sending", sendError: undefined }
        : message);
    }

    sendingMedia = true;
    mediaErrorMessage = "";

    try {
      await matrixSendMediaFile({
        roomId,
        filePath: mediaDraft.filePath,
        compressMedia: mediaDraft.compressMedia,
        caption: mediaDraft.caption || null,
      });

      mediaDraftsByLocalId.delete(localId);
      if (pendingMedia?.filePath === mediaDraft.filePath) {
        pendingMedia = null;
        mediaCaption = "";
        mediaEmojiPickerOpen = false;
      }
      mediaTranscodeProgress = null;

      messages = messages.map((message) => {
        if (message.localId !== localId) {
          return message;
        }

        return {
          ...message,
          sendState: undefined,
          localId: undefined,
        };
      });
    } catch (error) {
      const errorText = error instanceof Error ? error.message : "Failed to send media";
      const cancelled = errorText.toLowerCase().includes("cancelled by user");
      mediaTranscodeProgress = null;

      if (cancelled) {
        mediaErrorMessage = "";
        mediaDraftsByLocalId.delete(localId);
        void matrixRemovePastedFile(mediaDraft.filePath).catch(() => undefined);
        messages = messages.filter((message) => message.localId !== localId);
      } else {
        // Keep the caption/editor draft intact; the failed file bubble owns
        // this error and exposes retry/remove actions.
        messages = messages.map((message) => {
          if (message.localId !== localId) {
            return message;
          }

          return {
            ...message,
            sendState: "failed",
            sendError: errorText,
          };
        });
      }
    } finally {
      sendingMedia = false;
      activeMediaFilePath = "";
    }
  }

  async function cancelPendingMedia() {
    if (!pendingMedia) {
      return;
    }

    const roomId = $shellSelectedRoomId;
    const mediaDraft = pendingMedia;

    const failedLocalId = [...mediaDraftsByLocalId.entries()]
      .find(([, draft]) => draft.filePath === mediaDraft.filePath)?.[0];
    if (failedLocalId) {
      messages = messages.filter((message) => message.localId !== failedLocalId);
      mediaDraftsByLocalId.delete(failedLocalId);
    }

    if (sendingMedia && roomId && mediaDraft.compressMedia && mediaDraft.messageType !== "m.file") {
      try {
        await matrixCancelMediaTranscode({
          roomId,
          filePath: mediaDraft.filePath,
        });
      } catch {
        // Ignore cancellation command errors and close preview anyway.
      }
    }

    if (!sendingMedia) {
      void matrixRemovePastedFile(mediaDraft.filePath).catch(() => undefined);
    }

    mediaErrorMessage = "";
    mediaTranscodeProgress = null;
    activeMediaFilePath = "";
    pendingMedia = null;
    mediaCaption = "";
    mediaEmojiPickerOpen = false;
  }

  async function sendOptimisticMessage(
    roomId: string,
    request: MatrixSendChatMessageRequest,
    localId: string,
    optimistic: TimelineMessage,
  ) {
    sendingMessage = true;

    try {
      const response = await matrixSendChatMessage(request);

      if (response.eventId) {
        seenEventIds.add(response.eventId);
      }

      const confirmed: TimelineMessage = {
        ...optimistic,
        eventId: response.eventId,
        formattedBody: response.formattedBody || optimistic.formattedBody,
        sendState: undefined,
        localId: undefined,
      };

      const index = messages.findIndex(
        (message) => message.localId === localId || message.eventId === response.eventId,
      );

      if (index < 0) {
        // The timeline was rebuilt while the send was in flight (the room was
        // re-opened, the stream restarted). The message still went out, so it
        // is put back instead of vanishing.
        messages = [...messages, confirmed];
      } else {
        messages = messages.map((message, messageIndex) =>
          messageIndex === index ? confirmed : message,
        );
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to send message";
      composerErrorMessage = message;

      messages = messages.map((timelineMessage) => {
        if (timelineMessage.localId !== localId) {
          return timelineMessage;
        }

        return {
          ...timelineMessage,
          sendState: "failed",
        };
      });
    } finally {
      sendingMessage = false;
      queuePinTimelineToBottom(roomId, { force: true });
    }
  }

  function escapeMentionHtml(value: string): string {
    return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  }

  function formatBodyWithMentions(body: string, mentions: ComposerMention[]): string | null {
    const prepared = mentions
      .map((mention, index) => ({ ...mention, placeholder: `MATRIXXGRAMMENTION${index}TOKEN` }))
      .filter((mention) => body.slice(mention.start, mention.end) === `@${mention.displayName}`)
      .sort((left, right) => left.start - right.start);
    let markedBody = body;
    for (const mention of [...prepared].reverse()) {
      markedBody = markedBody.slice(0, mention.start) + mention.placeholder + markedBody.slice(mention.end);
    }
    let html = formatDraft(markedBody) ?? escapeMentionHtml(markedBody).replace(/\n/g, "<br>");
    for (const mention of prepared) {
      const href = `https://matrix.to/#/${encodeURIComponent(mention.userId)}`;
      const anchor = `<a class="matrix-mention" href="${href}">${escapeMentionHtml(`@${mention.displayName}`)}</a>`;
      html = html.replaceAll(mention.placeholder, anchor);
    }
    return html;
  }

  async function sendDraftMessage(rawDraft?: string, mentions: ComposerMention[] = []) {
    const roomId = $shellSelectedRoomId;
    if (!roomId) {
      return;
    }

    const sourceDraft = rawDraft ?? messageDraft;
    const leadingWhitespace = sourceDraft.length - sourceDraft.trimStart().length;
    const rawBody = sourceDraft.trim();
    if (!rawBody) {
      composerErrorMessage = "Message cannot be empty";
      return;
    }

    const body = rawBody;
    const inReplyToEventId = replyToMessage?.eventId ?? null;

    composerErrorMessage = "";
    messageDraft = "";
    replyToMessage = null;

    const normalizedMentions = mentions
      .map((mention) => ({ ...mention, start: mention.start - leadingWhitespace, end: mention.end - leadingWhitespace }))
      .filter((mention) => mention.start >= 0 && body.slice(mention.start, mention.end) === `@${mention.displayName}`);
    const formattedBody = normalizedMentions.length > 0
      ? formatBodyWithMentions(body, normalizedMentions)
      : formatDraft(body);
    const optimistic = { ...buildOptimisticMessage(body, inReplyToEventId), formattedBody };
    const localId = optimistic.localId;

    if (!localId) {
      return;
    }

    messages = [...messages, optimistic];
    queuePinTimelineToBottom(roomId, { force: true });

    await sendOptimisticMessage(
      roomId,
      {
        roomId,
        body,
        formattedBody,
        mentionedUserIds: [...new Set(normalizedMentions.map((mention) => mention.userId))],
        inReplyToEventId,
      },
      localId,
      optimistic,
    );
  }

  function updateReactionSummary(
    existing: MatrixReactionSummary[],
    key: string,
    userId: string,
    added: boolean,
  ): MatrixReactionSummary[] {
    const next = existing.map((reaction) => ({
      ...reaction,
      senders: [...reaction.senders],
    }));

    const index = next.findIndex((reaction) => reaction.key === key);
    if (index < 0) {
      if (!added) {
        return next;
      }

      next.push({ key, count: 1, senders: [userId], participants: [{ userId, timestamp: Date.now() }] });
      return next;
    }

    const reaction = next[index];
    const senderSet = new Set(reaction.senders);
    if (added) {
      senderSet.add(userId);
    } else {
      senderSet.delete(userId);
    }

    const senders = [...senderSet];
    if (senders.length === 0) {
      next.splice(index, 1);
      return next;
    }

    next[index] = {
      ...reaction,
      senders,
      participants: added
        ? [...(reaction.participants ?? []).filter(person => person.userId !== userId), { userId, timestamp: Date.now() }]
        : (reaction.participants ?? []).filter(person => person.userId !== userId),
      count: senders.length,
    };

    return next;
  }

  function clearReactionStateOverride(eventId: string, key: string) {
    const overrides = reactionStateOverrides.get(eventId);
    const override = overrides?.get(key);
    if (override) clearTimeout(override.timeout);
    overrides?.delete(key);
    if (overrides?.size === 0) reactionStateOverrides.delete(eventId);
  }

  function setReactionStateOverride(eventId: string, key: string, added: boolean) {
    clearReactionStateOverride(eventId, key);
    let overrides = reactionStateOverrides.get(eventId);
    if (!overrides) {
      overrides = new Map();
      reactionStateOverrides.set(eventId, overrides);
    }
    const token = Symbol();
    const timeout = setTimeout(() => {
      if (overrides?.get(key)?.token === token) clearReactionStateOverride(eventId, key);
    }, 10_000);
    overrides.set(key, { added, timeout, token });
  }

  function applyReactionStateOverrides(
    eventId: string,
    serverReactions: MatrixReactionSummary[],
  ): MatrixReactionSummary[] {
    const overrides = reactionStateOverrides.get(eventId);
    const userId = $shellCurrentUserId;
    if (!overrides?.size || !userId) return serverReactions;

    let reactions = serverReactions;
    for (const [key, override] of overrides) {
      const serverHasOwnReaction = serverReactions.some(
        (reaction) => reaction.key === key && reaction.senders.includes(userId),
      );
      if (serverHasOwnReaction === override.added) {
        clearReactionStateOverride(eventId, key);
        continue;
      }
      reactions = updateReactionSummary(reactions, key, userId, override.added);
    }
    return reactions;
  }

  async function handleToggleReaction(message: TimelineMessage, key: string) {
    const roomId = $shellSelectedRoomId;
    if (!roomId || !message.eventId) {
      return;
    }

    const normalizedKey = await normalizeReactionKey(key, $shellPickerCustomEmoji);
    const reactionKey = normalizedKey.trim();
    if (!reactionKey) {
      return;
    }

    const userId = $shellCurrentUserId;
    const targetEventId = message.eventId;
    let snapshot: TimelineMessage | null = null;
    let desiredAdded: boolean | null = null;

    if (userId) {
      const current = messages.find((candidate) => candidate.eventId === targetEventId);
      if (current) {
        snapshot = {
          ...current,
          reactions: current.reactions.map((reaction) => ({
            ...reaction,
            senders: [...reaction.senders],
          })),
        };

        const alreadyReacted = current.reactions.some(
          (reaction) => reaction.key === reactionKey && reaction.senders.includes(userId),
        );
        desiredAdded = !alreadyReacted;
        setReactionStateOverride(targetEventId, reactionKey, desiredAdded);

        messages = messages.map((candidate) => {
          if (candidate.eventId !== targetEventId) {
            return candidate;
          }

          return {
            ...candidate,
            reactions: updateReactionSummary(
              candidate.reactions,
              reactionKey,
              userId,
              !alreadyReacted,
            ),
          };
        });
      }
    }

    try {
      const response = await matrixToggleReaction({
        roomId,
        targetEventId,
        key: reactionKey,
      });

      if (!userId) {
        return;
      }
      if (desiredAdded !== null && response.added !== desiredAdded) {
        setReactionStateOverride(targetEventId, reactionKey, response.added);
      }

      messages = messages.map((candidate) => {
        if (candidate.eventId !== targetEventId) {
          return candidate;
        }

        return {
          ...candidate,
          reactions: updateReactionSummary(
            candidate.reactions,
            reactionKey,
            userId,
            response.added,
          ),
        };
      });
    } catch (error) {
      clearReactionStateOverride(targetEventId, reactionKey);
      if (snapshot) {
        messages = messages.map((candidate) => {
          if (candidate.eventId !== targetEventId) {
            return candidate;
          }

          return snapshot as TimelineMessage;
        });
      }

      composerErrorMessage = error instanceof Error ? error.message : "Failed to toggle reaction";
    }
  }

  async function retryMessage(message: TimelineMessage) {
    const roomId = $shellSelectedRoomId;
    if (!roomId || !message.localId || message.sendState !== "failed") {
      return;
    }

    if (mediaDraftsByLocalId.has(message.localId)) {
      await sendPendingMedia(message.localId);
      return;
    }

    composerErrorMessage = "";

    messages = messages.map((timelineMessage) => {
      if (timelineMessage.localId !== message.localId) {
        return timelineMessage;
      }

      return {
        ...timelineMessage,
        sendState: "sending",
        sendError: undefined,
      };
    });

    await sendOptimisticMessage(
      roomId,
      {
        roomId,
        body: message.body,
        inReplyToEventId: message.inReplyToEventId,
        mentionedUserIds: [],
      },
      message.localId,
      message,
    );
  }

  function removeFailedMessage(message: TimelineMessage) {
    if (!message.localId || message.sendState !== "failed") return;
    const removedDraft = mediaDraftsByLocalId.get(message.localId);
    mediaDraftsByLocalId.delete(message.localId);
    if (removedDraft) void matrixRemovePastedFile(removedDraft.filePath).catch(() => undefined);
    messages = messages.filter((candidate) => candidate.localId !== message.localId);
    if (pendingMedia?.filePath === removedDraft?.filePath) {
      pendingMedia = null;
      mediaCaption = "";
      mediaErrorMessage = "";
    }
  }

  function handleReplyToMessage(message: TimelineMessage) {
    if (!message.eventId) {
      return;
    }

    replyToMessage = message;
    composerFocusNonce += 1;
  }

  async function handleDeleteMessage(message: TimelineMessage) {
    const roomId = $shellSelectedRoomId;
    if (!roomId || !message.eventId || message.isDeleted || message.sender !== $shellCurrentUserId) return;
    if (!window.confirm("Delete this message for everyone? This cannot be undone.")) return;

    try {
      await matrixRedactMessage({ roomId, eventId: message.eventId });
      messages = messages.map((candidate) => candidate.eventId === message.eventId
        ? { ...candidate, body: "", formattedBody: null, messageType: null, imageUrl: null, thumbnailUrl: null, reactions: [], customEmojis: [], isDeleted: true }
        : candidate);
    } catch (error) {
      composerErrorMessage = error instanceof Error ? error.message : String(error);
    }
  }

  /// Forward a message: pick the room it goes to, then send its text there.
  function handleForwardMessage(message: TimelineMessage) {
    forwardError = "";
    forwardingMessage = message;
  }

  async function handleForwardToRoom(roomId: string) {
    const message = forwardingMessage;
    if (!message || forwarding) {
      return;
    }

    forwarding = true;
    forwardError = "";

    try {
      const response = await matrixSendChatMessage({
        roomId,
        body: message.body,
        inReplyToEventId: null,
        mentionedUserIds: [],
        // Forwarding keeps the formatting of the original message. The reply
        // quote block is left out: it belongs to the conversation the
        // message came from.
        formattedBody: message.formattedBody
          ? stripMxReplyBlock(message.formattedBody)
          : null,
      });

      if (response.eventId) {
        seenEventIds.add(response.eventId);
      }

      forwardingMessage = null;
    } catch (error) {
      forwardError = error instanceof Error ? error.message : "Failed to forward the message";
    } finally {
      forwarding = false;
    }
  }

  function clearReplyToMessage() {
    replyToMessage = null;
  }

  function resetRoomState() {
    initialLoadGeneration++;
    clearTimeout(readTimer);
    unreadBoundaryId = null; unreadRemaining = 0; unreadTotal = 0;
    readPositionError = "";
    readMarkerId = null; initialLatestTimestamp = 0;
    consumedUnread.clear(); newUnread.clear();
    readPositionReady = false; readStateConfirmed = false; initialPositionPending = true;
    positioningInitial = false; nextTo = null; loadingNewer = false;
    readingContext = false; jumpRequested = false; highestReadTimestamp = 0;
    lastScrollTop = 0;
    restoringTimelinePosition = false;
    isScrolledToBottom = true;
    programmaticScrollUntil = 0;
    activeStreamId = "";
    activeLoadKind = null;
    streamMessageCount = 0;
    seenEventIds.clear();
    messageDraft = "";
    sendingMessage = false;
    composerErrorMessage = "";
    pendingMedia = null;
    mediaCaption = "";
    mediaEmojiPickerOpen = false;
    mediaErrorMessage = "";
    activeMediaFilePath = "";
    mediaTranscodeProgress = null;
    replyToMessage = null;
    pendingRoomMessages = null;
    pendingOlderMessages = [];
    messages = [];
    nextFrom = null;
    pendingRoomViewportAnchor = null;
  }

  async function loadMessages(roomId: string, jumpToEnd = false) {
    loadingMessages = true;
    errorMessage = "";
    pendingOlderMessages = [];

    const streamId = createStreamId();
    activeStreamId = streamId;
    activeLoadKind = "initial";
    streamMessageCount = 0;
    const generation = ++initialLoadGeneration;
    const startedAt = performance.now();
    const savedRoomAnchor = jumpToEnd ? null : pendingRoomViewportAnchor;

    // Resolve the read marker independently of the message stream so cached
    // messages paint immediately. But never position/mark the latest page as
    // read until the last-read context has actually been resolved.
    void (async () => {
      if (jumpToEnd) {
        readStateConfirmed = true;
        readPositionReady = true;
        return;
      }
      let read: Awaited<ReturnType<typeof matrixGetRoomReadState>>;
      try {
        read = await matrixGetRoomReadState(roomId);
      } catch (error) {
        if ($shellSelectedRoomId !== roomId || initialLoadGeneration !== generation) return;
        // A saved viewport anchor is independent from Matrix read-receipt state;
        // still position from it even if that request is temporarily unavailable.
        readPositionReady = Boolean(pendingRoomViewportAnchor);
        readStateConfirmed = false;
        if (!pendingRoomViewportAnchor) initialPositionPending = false;
        readPositionError = error instanceof Error ? `Couldn't restore the last-read position: ${error.message}` : "Couldn't restore the last-read position";
        return;
      }
      if ($shellSelectedRoomId !== roomId || initialLoadGeneration !== generation) return;
      readMarkerId = read.eventId ?? null;
      readPositionError = "";
      unreadTotal = read.unreadCount;
      unreadRemaining = unreadTotal;
      initialLatestTimestamp = read.latestTimestamp ?? Date.now();
      if (readMarkerId && unreadTotal > 0 && !pendingRoomViewportAnchor) {
        let context: Awaited<ReturnType<typeof matrixGetReadContext>>;
        try {
          context = await matrixGetReadContext(roomId, readMarkerId);
        } catch (error) {
          if ($shellSelectedRoomId !== roomId || initialLoadGeneration !== generation) return;
          readPositionReady = false;
          readStateConfirmed = false;
          initialPositionPending = false;
          readPositionError = error instanceof Error ? `Couldn't open the last-read position: ${error.message}` : "Couldn't open the last-read position";
          return;
        }
        if ($shellSelectedRoomId !== roomId || initialLoadGeneration !== generation) return;
        if (!context.messages.length) {
          readPositionReady = false;
          readStateConfirmed = false;
          initialPositionPending = false;
          readPositionError = "Couldn't find the last-read message in this room. Retry to try again.";
          return;
        }
        if (initialPositionPending) {
          const latestMessages = messages;
          messages = context.messages;
          seenEventIds.clear();
          for (const message of messages) if (message.eventId) seenEventIds.add(message.eventId);
          nextFrom = context.nextFrom ?? null;
          nextTo = context.nextTo ?? null;
          readingContext = Boolean(nextTo);
          if (latestMessages.length) {
            const latest = { roomId, nextFrom: null, messages: latestMessages };
            if (readingContext) pendingRoomMessages = latest;
            else applySelectedRoomMessagesNow(latest);
          }
        }
      }
      readStateConfirmed = true;
      readPositionReady = true;
      console.debug(`Read position ready in ${Math.round(performance.now() - startedAt)}ms`);
    })();

    // Defense-in-depth: if the backend never emits a terminal `done` event
    // (e.g. a stream failure), clear the loading state after a timeout so the
    // UI never hangs in an infinite loading loop.
    const timeoutHandle = setTimeout(() => {
      if (activeStreamId !== streamId) {
        return;
      }

      loadingMessages = false;
      errorMessage = "Loading messages is taking longer than expected. You can retry.";
    }, MESSAGE_LOAD_TIMEOUT_MS);

    try {
      if (savedRoomAnchor) {
        // Buffer room-sync snapshots while its saved viewport window is loading.
        readingContext = true;
        nextTo = "__restoring_saved_anchor__";
        try {
          const context = await matrixGetReadContext(roomId, savedRoomAnchor.eventId);
          if ($shellSelectedRoomId !== roomId || initialLoadGeneration !== generation) {
            clearTimeout(timeoutHandle);
            return;
          }
          if (context.messages.some((message) => message.eventId === savedRoomAnchor.eventId)) {
            messages = context.messages;
            seenEventIds.clear();
            for (const message of messages) if (message.eventId) seenEventIds.add(message.eventId);
            nextFrom = context.nextFrom ?? null;
            nextTo = context.nextTo ?? null;
            readingContext = Boolean(nextTo);
            loadingMessages = false;
            activeStreamId = "";
            activeLoadKind = null;
            streamMessageCount = 0;
            clearTimeout(timeoutHandle);
            flushPendingRoomMessages();
            return;
          }
        } catch {
          // Fall back to the normal cached latest page if the saved anchor is unavailable.
        }
        readingContext = false;
        nextTo = null;
      }

      await matrixStreamChatMessages({
        roomId,
        from: null,
        limit: 50,
        streamId,
        loadKind: "initial",
      });

      if ($shellSelectedRoomId !== roomId || activeStreamId !== streamId) {
        clearTimeout(timeoutHandle);
        return;
      }
    } catch (error) {
      clearTimeout(timeoutHandle);

      if (activeStreamId !== streamId) {
        return;
      }

      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      errorMessage = error instanceof Error ? error.message : "Failed to stream messages";
    }
  }

  function trackNewUnread(incoming: MatrixChatMessage[]) {
    for (const message of incoming) {
      if (isUnreadMessage(message) && message.eventId && message.sender !== $shellCurrentUserId &&
          (message.timestamp ?? 0) > initialLatestTimestamp) newUnread.add(message.eventId);
    }
    unreadRemaining = Math.max(0, unreadTotal + newUnread.size - consumedUnread.size);
  }

  async function positionInitialRead() {
    const roomId = $shellSelectedRoomId;
    if (!readPositionReady || !initialPositionPending || positioningInitial || !messages.length || !timelineElement) return;
    positioningInitial = true;
    try {
      const index = jumpRequested ? -1 : firstUnreadIndex(messages, readMarkerId, unreadTotal, $shellCurrentUserId);
      unreadBoundaryId = index >= 0 ? messages[index].eventId ?? null : null;
      await tick();
      if ($shellSelectedRoomId !== roomId || !timelineElement) return;
      beginProgrammaticScroll();
      const savedAnchor = pendingRoomViewportAnchor;
      const savedAnchorElement = savedAnchor
        ? timelineElement.querySelector<HTMLElement>(
            `[data-message-event-id="${savedAnchor.eventId.replaceAll('"', '\\"')}"]`,
          )
        : null;
      const divider = timelineElement.querySelector<HTMLElement>("[data-unread-divider]");
      if (savedAnchor && savedAnchorElement) {
        timelineElement.scrollTop += savedAnchorElement.getBoundingClientRect().top
          - timelineElement.getBoundingClientRect().top - savedAnchor.offset;
      } else if (divider) {
        timelineElement.scrollTop += divider.getBoundingClientRect().top - timelineElement.getBoundingClientRect().top - 16;
      } else {
        timelineElement.scrollTop = timelineElement.scrollHeight;
      }
      pendingRoomViewportAnchor = null;
      lastScrollTop = timelineElement.scrollTop;
      isScrolledToBottom = !nextTo && distanceToBottom(timelineElement) <= NEAR_BOTTOM_PX;
      initialPositionPending = false;
      scheduleReadVisible();
      if (nextTo && distanceToBottom(timelineElement) < timelineElement.clientHeight * 1.5) void loadNewer();
    } finally { if ($shellSelectedRoomId === roomId) positioningInitial = false; }
  }

  function scheduleReadVisible() {
    clearTimeout(readTimer);
    readTimer = setTimeout(() => { void markVisibleRead(); }, 220);
  }
  async function markVisibleRead() {
    const element = timelineElement, roomId = $shellSelectedRoomId;
    if (!element || !roomId || !readPositionReady || !readStateConfirmed || initialPositionPending || restoringTimelinePosition ||
        document.hidden || !document.hasFocus() || readRequestInFlight) return;
    const viewport = element.getBoundingClientRect();
    const messageByEventId = new Map(messages.filter(message => message.eventId).map(message => [message.eventId!, message]));
    const visible = Array.from(element.querySelectorAll<HTMLElement>("[data-message-event-id]"))
      .filter(node => {
        const message = messageByEventId.get(node.dataset.messageEventId ?? "");
        if (!message || !isUnreadMessage(message)) return false;
        const rect = node.getBoundingClientRect();
        return rect.bottom <= viewport.bottom + 2 && rect.bottom > viewport.top;
      });
    const ids = new Set(visible.map(node => node.dataset.messageEventId));
    let last = -1;
    for (let i = 0; i < messages.length; i++) if (ids.has(messages[i].eventId ?? "")) last = i;
    if (last < 0) return;
    const message = messages[last];
    const markerIndex = readMarkerId ? messages.findIndex(item => item.eventId === readMarkerId) : -1;
    if (markerIndex >= 0 && last < markerIndex) return;
    if (!message.eventId || !message.timestamp || message.timestamp < highestReadTimestamp ||
        (message.timestamp === highestReadTimestamp && !(!nextTo && last === messages.length - 1 && unreadRemaining > 0))) return;
    const start = firstUnreadIndex(messages, readMarkerId, unreadTotal, $shellCurrentUserId);
    if (start >= 0) for (let i = start; i <= last; i++) {
      const item = messages[i];
      if (isUnreadMessage(item) && item.eventId && item.sender !== $shellCurrentUserId) consumedUnread.add(item.eventId);
    }
    trackNewUnread(messages);
    if (!nextTo && last === messages.length - 1) unreadRemaining = 0;
    if (unreadRemaining === 0) unreadBoundaryId = null;
    const remaining = unreadRemaining;
    const previousCount = unreadTotal + newUnread.size;
    const reachedLatest = !nextTo && last === messages.length - 1;
    readRequestInFlight = true;
    try {
      await matrixMarkRoomRead({ roomId, eventId: message.eventId, timestamp: message.timestamp,
        remaining, previousCount, latestTimestamp: initialLatestTimestamp });
      if ($shellSelectedRoomId === roomId) {
        highestReadTimestamp = message.timestamp;
        if (reachedLatest) {
          unreadTotal = 0; newUnread.clear(); consumedUnread.clear();
          initialLatestTimestamp = Math.max(initialLatestTimestamp, message.timestamp);
          unreadRemaining = 0;
          unreadBoundaryId = null;
        }
        shellChats.update(rooms => rooms.map(room => room.roomId === roomId ? { ...room, unreadCount: remaining } : room));
      }
    } catch (error) { console.debug("Read receipt will retry on next scroll", error); }
    finally {
      readRequestInFlight = false;
      if ($shellSelectedRoomId !== roomId || highestReadTimestamp === message.timestamp) scheduleReadVisible();
    }
  }

  async function loadNewer() {
    const roomId = $shellSelectedRoomId, from = nextTo;
    if (!roomId || !from || loadingNewer || initialPositionPending) return;
    loadingNewer = true;
    try {
      const response = await matrixGetNewerMessages(roomId, from);
      if ($shellSelectedRoomId !== roomId || nextTo !== from) return;
      applySelectedRoomMessagesNow({ roomId, nextFrom: null, messages: response.messages });
      nextTo = response.nextTo ?? null;
      if (!nextTo) { readingContext = false; flushPendingRoomMessages(); }
      await tick();
      scheduleReadVisible();
    } catch (error) { errorMessage = error instanceof Error ? error.message : "Failed to load newer messages"; }
    finally { if ($shellSelectedRoomId === roomId) loadingNewer = false; }
  }

  async function loadOlderForReplyJump(roomId: string, eventId: string) {
    const targetIndex = messages.findIndex((message) => message.eventId === eventId);
    if (targetIndex < 0 || targetIndex > REPLY_JUMP_OLDER_EDGE_MESSAGES || !nextFrom || loadingMessages) return;
    // A programmatic reply jump suppresses normal scroll-driven pagination.
    // Fetch the next older page here when the target sits at the loaded edge.
    beginProgrammaticScroll();
    await loadOlder();
    if ($shellSelectedRoomId !== roomId) return;
  }

  async function loadJumpContext(eventId: string): Promise<boolean> {
    const roomId = $shellSelectedRoomId;
    if (!roomId) return false;
    if (messages.some((message) => message.eventId === eventId)) {
      await loadOlderForReplyJump(roomId, eventId);
      return $shellSelectedRoomId === roomId && messages.some((message) => message.eventId === eventId);
    }

    try {
      const context = await matrixGetReadContext(roomId, eventId);
      if ($shellSelectedRoomId !== roomId || !context.messages.some((message) => message.eventId === eventId)) {
        return false;
      }

      beginProgrammaticScroll();
      const pendingLocalMessages = messages.filter((message) => message.sendState && !message.eventId);
      messages = [...context.messages, ...pendingLocalMessages];
      seenEventIds.clear();
      for (const message of messages) if (message.eventId) seenEventIds.add(message.eventId);
      nextFrom = context.nextFrom ?? null;
      nextTo = context.nextTo ?? null;
      readingContext = Boolean(nextTo);
      const unreadIndex = firstUnreadIndex(messages, readMarkerId, unreadTotal, $shellCurrentUserId || "");
      unreadBoundaryId = unreadIndex >= 0 ? messages[unreadIndex]?.eventId ?? null : null;
      isScrolledToBottom = false;
      await tick();
      await loadOlderForReplyJump(roomId, eventId);
      if (!nextTo) flushPendingRoomMessages();
      return $shellSelectedRoomId === roomId && Boolean(timelineElement?.querySelector(
        `[data-message-event-id="${eventId.replaceAll('"', '\\"')}"]`,
      ));
    } catch (error) {
      console.debug("Could not load reply context", error);
      return false;
    }
  }

  function jumpToLatest() {
    const roomId = $shellSelectedRoomId;
    if (!roomId) return;
    pendingRoomViewportAnchor = null;
    if (nextTo) {
      nextTo = null; readingContext = false; pendingRoomMessages = null;
      messages = []; seenEventIds.clear(); nextFrom = null;
      unreadBoundaryId = null; initialPositionPending = true; jumpRequested = true;
      void loadMessages(roomId, true);
    } else {
      queuePinTimelineToBottom(roomId, { force: true });
      scheduleReadVisible();
    }
  }

  function jumpToNextAttention() {
    if (!timelineElement || unreadAttentionMessages.length === 0) return;
    const viewport = timelineElement.getBoundingClientRect();
    const visibleNodes = Array.from(timelineElement.querySelectorAll<HTMLElement>("[data-message-event-id]"))
      .filter((node) => {
        const rect = node.getBoundingClientRect();
        return rect.bottom > viewport.top && rect.top < viewport.bottom;
      });
    const bottomVisibleNode = visibleNodes.at(-1);
    const bottomVisibleIndex = bottomVisibleNode
      ? messages.findIndex((message) => message.eventId === bottomVisibleNode.dataset.messageEventId)
      : -1;
    const next = unreadAttentionMessages.find((message) =>
      messages.findIndex((candidate) => candidate.eventId === message.eventId) > bottomVisibleIndex,
    ) ?? unreadAttentionMessages[0];
    if (next.eventId) attentionJumpRequest = { eventId: next.eventId, nonce: ++attentionJumpNonce };
  }

  function retryInitialLoad() {
    const roomId = $shellSelectedRoomId;
    if (!roomId) return;
    initialPositionPending = true;
    positioningInitial = false;
    readPositionReady = false;
    readStateConfirmed = false;
    readPositionError = "";
    errorMessage = "";
    void loadMessages(roomId);
  }
  function jumpToNextReaction() {
    const roomId = $shellSelectedRoomId;
    const eventId = unreadReactionTargets[0];
    if (!roomId || !eventId) return;
    reactionJumpRequest = { roomId, eventId, nonce: ++reactionJumpNonce };
  }
  function finishReactionJump(roomId: string, eventId: string) {
    consumeReactionTarget(roomId, eventId);
    if (reactionJumpRequest?.roomId === roomId && reactionJumpRequest.eventId === eventId) {
      reactionJumpRequest = null;
    }
  }
  $effect(() => {
    const roomId = $shellSelectedRoomId;
    if (reactionJumpRequest && reactionJumpRequest.roomId !== roomId) reactionJumpRequest = null;
  });
  $effect(() => {
    if (messages.length && readPositionReady) {
      if (initialPositionPending) void positionInitialRead();
      else { trackNewUnread(messages); scheduleReadVisible(); }
    }
  });

  async function loadOlder() {
    const selectedRoomId = $shellSelectedRoomId;
    if (!selectedRoomId || !nextFrom || loadingMessages) {
      return;
    }

    loadingMessages = true;
    errorMessage = "";
    pendingOlderMessages = [];

    const streamId = createStreamId();
    activeStreamId = streamId;
    activeLoadKind = "older";
    streamMessageCount = 0;

    try {
      await matrixStreamChatMessages({
        roomId: selectedRoomId,
        from: nextFrom,
        limit: 30,
        streamId,
        loadKind: "older",
      });
    } catch (error) {
      if (activeStreamId !== streamId) {
        return;
      }

      flushPendingOlderMessages();
      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      errorMessage = error instanceof Error ? error.message : "Failed to stream older messages";
    }
  }

  function selectedRoomEncrypted(): boolean {
    const selectedRoomId = $shellSelectedRoomId;
    const selectedRoom = $shellChats.find((chat) => chat.roomId === selectedRoomId);
    return selectedRoom?.encrypted ?? false;
  }

  /// A direct chat shows no sender names: there is only one other person.
  function selectedRoomIsGroup(): boolean {
    const selectedRoomId = $shellSelectedRoomId;
    const selectedRoom = $shellChats.find((chat) => chat.roomId === selectedRoomId);
    if (!selectedRoom) {
      return true;
    }

    return !selectedRoom.isDirect;
  }

  function selectedRoomIsDirect(): boolean {
    const selectedRoom = $shellChats.find((chat) => chat.roomId === $shellSelectedRoomId);
    return Boolean(selectedRoom?.isDirect);
  }
</script>

{#snippet mediaCaptionEmojiIcon()}
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    class="size-6"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="10" />
    <path d="M8 14s1.5 2 4 2 4-2 4-2" />
    <line x1="9" y1="9" x2="9.01" y2="9" />
    <line x1="15" y1="9" x2="15.01" y2="9" />
  </svg>
{/snippet}
<svelte:window onfocus={scheduleReadVisible} />
<svelte:document onvisibilitychange={scheduleReadVisible} />

{#if roomSettingsDialogRoomId}
  {@const dialogRoom = $shellChats.find((chat) => chat.roomId === roomSettingsDialogRoomId)}
  {#if dialogRoom}
    <RoomSettingsDialog
      open={true}
      roomId={dialogRoom.roomId}
      roomName={dialogRoom.displayName}
      isSpace={dialogRoom.kind === "space"}
      isDirect={dialogRoom.isDirect}
      onClose={() => (roomSettingsDialogRoomId = null)}
    />
  {/if}
{/if}

{#if page.url.searchParams.get("view") === "communities"}
  <CommunityExplorer />
{:else if !$shellSelectedRoomId}
  <section class="grid h-full place-items-center bg-panel-alt">
    <div class="max-w-sm px-6 text-center">
      <h1 class="text-xl font-semibold">MatriXgram</h1>
      <p class="mt-2 text-sm text-secondary">
        Pick a chat from the list on the left to start reading and writing.
      </p>
    </div>
  </section>
{:else}
  <div class="flex flex-col h-full min-w-0 overflow-x-hidden">
    <MessageTimeline
      {messages}
      roomId={$shellSelectedRoomId || ""}
      selectedRoomId={$shellSelectedRoomId}
      {unreadBoundaryId}
      unreadCount={unreadRemaining}
      showAttentionIndicator={hasUnreadAttention}
      attentionCount={unreadAttentionMessages.length}
      {attentionJumpRequest}
      {reactionJumpRequest}
      reactionCount={unreadReactionTargets.length}
      onJumpToReaction={jumpToNextReaction}
      onReactionJumped={finishReactionJump}
      showJumpToLatest={!initialPositionPending && (!isScrolledToBottom || Boolean(nextTo) || unreadRemaining > 0)}
      onJumpToLatest={jumpToLatest}
      onJumpToAttention={jumpToNextAttention}
      currentUserId={$shellCurrentUserId || null}
      pickerCustomEmoji={$shellPickerCustomEmoji}
      roomEncrypted={selectedRoomEncrypted()}
      roomName={$shellChats.find((chat) => chat.roomId === $shellSelectedRoomId)?.displayName ?? ""}
      isDirect={selectedRoomIsDirect()}
      showSenderNames={selectedRoomIsGroup()}
      timelineVisible={!initialPositionPending}
      loadingMessages={loadingMessages || (!roomUpdatesReady && !roomUpdatesError) || (roomUpdatesReady && previousSelectedRoomId !== $shellSelectedRoomId)}
      {activeLoadKind}
      {streamMessageCount}
      error={roomUpdatesError || readPositionError || errorMessage}
      isSending={sendingMessage || sendingMedia}
      onTimelineElementChange={(element) => timelineElement = element}
      onScroll={handleTimelineScroll}
      onProgrammaticScroll={beginProgrammaticScroll}
      onRetryLoad={() => { if (!roomUpdatesReady) retryRoomUpdates(); else retryInitialLoad(); }}
      onOpenRoomSettings={(roomIdToOpen) => (roomSettingsDialogRoomId = roomIdToOpen)}
      onRetryMessage={retryMessage}
      onRemoveFailedMessage={removeFailedMessage}
      onToggleReaction={handleToggleReaction}
      onReplyToMessage={handleReplyToMessage}
      onForwardMessage={handleForwardMessage}
      onDeleteMessage={handleDeleteMessage}
      onLoadJumpContext={loadJumpContext}
    />

    <div class="composer-dock" bind:this={composerDockElement}>
      <MessageComposer
        draft={messageDraft}
        roomId={$shellSelectedRoomId}
        error={composerErrorMessage}
        isSending={sendingMessage}
        isDisabled={!$shellSelectedRoomId}
        pickerCustomEmoji={$shellPickerCustomEmoji}
        {replyToMessage}
        focusNonce={composerFocusNonce}
        placeholder={$shellSelectedRoomId ? "Write a message..." : "Select a room to compose"}
        onSubmit={sendDraftMessage}
        onDraftChange={(d) => messageDraft = d}
        onChooseAttachment={chooseAttachment}
        onPasteAttachmentPath={createPendingMedia}
        onClearReply={clearReplyToMessage}
      />
    </div>

    {#if forwardingMessage}
  <ForwardRoomPicker
    open={true}
    message={forwardingMessage}
    rooms={$shellChats}
    error={forwardError}
    onClose={() => {
      forwardingMessage = null;
      forwardError = "";
    }}
    onForward={handleForwardToRoom}
  />
{/if}

{#if pendingMedia}
  {@const pendingMediaIsVideo = pendingMedia.messageType === "m.video"}
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
        role="button"
        tabindex="0"
        onclick={handlePendingMediaBackdropClick}
        onkeydown={handlePendingMediaBackdropKeydown}
      >
        <div class="max-h-[92vh] w-full overflow-y-auto bg-surface-100-900 shadow-2xl {pendingMedia.messageType === 'm.file' ? 'max-w-md rounded-[8px] border border-surface-300-700' : 'max-w-md rounded-[8px]'}">
          <div class="flex items-center justify-between border-b border-surface-300-700 px-3 py-2">
            <div>
              <p class={"text-sm font-medium text-surface-900-100"}>{pendingMedia.messageType === "m.image" ? "Send image" : pendingMedia.messageType === "m.file" ? "Send file" : "Send video"}</p>
            </div>
          </div>

          <div class={pendingMedia.messageType === "m.file" ? "flex flex-col gap-3 px-1 py-2" : "flex flex-col gap-2 p-1"}>
            <div class={pendingMedia.messageType === "m.file" ? "flex items-center gap-2" : "relative flex max-h-60 w-full items-center justify-center overflow-hidden rounded-[8px]"}>
              {#if pendingMedia.messageType === "m.image"}
                <button
                  class="absolute right-1.5 top-1.5 z-10 grid size-7 place-items-center rounded-[6px] bg-black/45 text-lg leading-none text-white transition hover:bg-black/65"
                  type="button"
                  aria-label="Close image preview"
                  title="Close"
                  onclick={cancelPendingMedia}
                >×</button>
                <img
                  src={convertFileSrc(pendingMedia.filePath)}
                  alt={pendingMedia.fileName}
                  class="block max-h-60 w-full object-contain"
                />
              {:else if pendingMedia.messageType === "m.video"}
                <button
                  class="absolute right-1.5 top-1.5 z-10 grid size-7 place-items-center rounded-[6px] bg-black/45 text-lg leading-none text-white transition hover:bg-black/65"
                  type="button"
                  aria-label="Close video preview"
                  title="Close"
                  onclick={cancelPendingMedia}
                >×</button>
                <!-- svelte-ignore a11y_media_has_caption -->
                <video
                  src={convertFileSrc(pendingMedia.filePath)}
                  controls
                  playsinline
                  class="block max-h-60 w-full rounded-[8px] bg-black object-contain"
                ></video>
              {:else}
                {@const badge = fileBadge(pendingMedia.fileName)}
                <div class={`grid size-14 shrink-0 place-items-center rounded-lg text-sm font-bold ${badge.color}`} aria-label={`${fileExtension(pendingMedia.fileName)} file`}>
                  {badge.label}
                </div>
                <div class="min-w-0 flex-1">
                  <p class="break-all text-sm font-medium text-surface-900-100">{pendingMedia.fileName}</p>
                  <p class="mt-1 text-xs text-surface-600-400">{formatFileSize(pendingMedia.fileSize)}</p>
                </div>
                <button
                  type="button"
                  class="grid size-7 shrink-0 place-items-center text-lg leading-none text-surface-600-400 transition hover:text-surface-900-100"
                  aria-label="Remove file"
                  title="Remove file"
                  onclick={cancelPendingMedia}
                >×</button>
              {/if}
            </div>

            <p class="px-2 text-xs text-surface-600-400">
              {#if pendingMedia.checkingLimit}
                Checking this homeserver’s upload limit…
              {:else if pendingMedia.fileSize !== null && pendingMedia.maxUploadSize !== null}
                {#if pendingMedia.fileSize > pendingMedia.maxUploadSize && pendingMedia.compressMedia}
                  Original: {formatFileSize(pendingMedia.fileSize)}; compressed output will be checked before upload ({formatFileSize(pendingMedia.maxUploadSize)} maximum)
                {:else}
                  {formatFileSize(pendingMedia.fileSize)} of {formatFileSize(pendingMedia.maxUploadSize)} maximum
                {/if}
              {:else}
                Upload limit unavailable
              {/if}
            </p>

            <div class={pendingMedia.messageType === "m.file" ? "space-y-3" : "space-y-2"}>
              {#if pendingMedia.messageType !== "m.file"}
                <label class={"flex items-center gap-2 px-1 text-sm"}>
                  <input type="checkbox" bind:checked={pendingMedia.compressMedia} class="mt-0.5" />
                  <span class="font-medium">Compress before sending</span>
                  {#if pendingMedia.messageType !== "m.image"}
                    <span class="block text-sm text-surface-600-400">
                      Videos become VP9 WebM.
                    </span>
                  {/if}
                </label>
              {/if}
              {#if pendingMedia.messageType === "m.image"}
                  <div class="group mb-2 flex min-h-12 items-end gap-1 border-b-[1.5px] border-surface-500-500 px-1.5 focus-within:border-primary-500">
                    <div class="flex min-w-0 flex-1 flex-col">
                      <span class="pt-1 text-xs text-surface-600-400">Caption</span>
                      <input
                        bind:this={mediaCaptionInput}
                        bind:value={mediaCaption}
                        type="text"
                        maxlength="2000"
                        aria-label="Caption"
                        class="media-caption-input min-w-0 w-full border-0 bg-transparent px-0 pb-1.5 text-sm text-surface-900-100 shadow-none outline-none ring-0 focus:border-0 focus:ring-0 focus-visible:outline-none"
                        onkeydown={(event) => {
                          if (event.key === "Enter" && !event.shiftKey) {
                            event.preventDefault();
                            if (!sendingMedia && $shellSelectedRoomId) void sendPendingMedia();
                          }
                        }}
                      />
                    </div>
                    <EmojiPickerPopover
                      bind:open={mediaEmojiPickerOpen}
                      icon={mediaCaptionEmojiIcon}
                      buttonClass="media-caption-emoji composer-tool btn shrink-0"
                      wrapperClass="media-caption-emoji-wrap relative flex h-[34px] w-[38px] shrink-0 items-center justify-center"
                      popoverClass="rounded-xl border border-hairline bg-panel p-1 shadow-lg"
                      pickerStyle="width: min(280px, calc(100vw - 32px)); height: 260px; --emoji-size: 1.35rem; --category-emoji-size: 1rem; --num-columns: 7;"
                      pickerCustomEmoji={$shellPickerCustomEmoji}
                      onSelect={insertMediaCaptionEmoji}
                    />
                    <button
                      type="button"
                      class="composer-send btn shrink-0"
                      disabled={sendingMedia || !$shellSelectedRoomId || pendingMedia.checkingLimit || !!pendingMedia.limitError || (!pendingMedia.compressMedia && pendingMedia.fileSize !== null && pendingMedia.maxUploadSize !== null && pendingMedia.fileSize > pendingMedia.maxUploadSize)}
                      title={String(pendingMedia.messageType) === "m.video" ? "Send video" : "Send image"}
                      aria-label={String(pendingMedia.messageType) === "m.video" ? "Send video" : "Send image"}
                      onclick={() => void sendPendingMedia()}
                    >
                      {#if sendingMedia}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-6 animate-spin" aria-hidden="true">
                          <path d="M21 12a9 9 0 1 1-6.22-8.56" />
                        </svg>
                      {:else}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-6" aria-hidden="true">
                          <line x1="22" y1="2" x2="11" y2="13" />
                          <polygon points="22 2 15 22 11 13 2 9 22 2" />
                        </svg>
                      {/if}
                    </button>
                  </div>
              {:else}
                <div class="group mb-2 flex min-h-12 items-end gap-1 border-b-[1.5px] border-surface-500-500 px-1.5 focus-within:border-primary-500">
                  <div class="flex min-w-0 flex-1 flex-col">
                    <span class="pt-1 text-xs text-surface-600-400">Caption</span>
                    <input
                      bind:this={mediaCaptionInput}
                      bind:value={mediaCaption}
                      type="text"
                      maxlength="2000"
                      aria-label="Caption"
                      class="media-caption-input min-w-0 w-full border-0 bg-transparent px-0 pb-1.5 text-sm text-surface-900-100 shadow-none outline-none ring-0 focus:border-0 focus:ring-0 focus-visible:outline-none"
                      onkeydown={(event) => {
                        if (event.key === "Enter" && !event.shiftKey) {
                          event.preventDefault();
                          if (!sendingMedia && $shellSelectedRoomId) void sendPendingMedia();
                        }
                      }}
                    />
                  </div>
                  <EmojiPickerPopover
                    bind:open={mediaEmojiPickerOpen}
                    icon={mediaCaptionEmojiIcon}
                    buttonClass="media-caption-emoji composer-tool btn shrink-0"
                    wrapperClass="media-caption-emoji-wrap relative flex h-[34px] w-[38px] shrink-0 items-center justify-center"
                    popoverClass="rounded-xl border border-hairline bg-panel p-1 shadow-lg"
                    pickerStyle="width: min(280px, calc(100vw - 32px)); height: 260px; --emoji-size: 1.35rem; --category-emoji-size: 1rem; --num-columns: 7;"
                    pickerCustomEmoji={$shellPickerCustomEmoji}
                    onSelect={insertMediaCaptionEmoji}
                  />
                  <button
                    type="button"
                    class="composer-send btn shrink-0"
                    disabled={sendingMedia || !$shellSelectedRoomId || pendingMedia.checkingLimit || !!pendingMedia.limitError || (!pendingMedia.compressMedia && pendingMedia.fileSize !== null && pendingMedia.maxUploadSize !== null && pendingMedia.fileSize > pendingMedia.maxUploadSize)}
                    title={pendingMedia.messageType === "m.video" ? "Send video" : "Send file"}
                    aria-label={pendingMedia.messageType === "m.video" ? "Send video" : "Send file"}
                    onclick={() => void sendPendingMedia()}
                  >
                    {#if sendingMedia}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-6 animate-spin" aria-hidden="true">
                        <path d="M21 12a9 9 0 1 1-6.22-8.56" />
                      </svg>
                    {:else}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-6" aria-hidden="true">
                        <line x1="22" y1="2" x2="11" y2="13" />
                        <polygon points="22 2 15 22 11 13 2 9 22 2" />
                      </svg>
                    {/if}
                  </button>
                </div>
              {/if}

              {#if sendingMedia}
                <div class="space-y-2 rounded-lg border border-surface-300-700 p-3">
                  <div class="flex items-center justify-between gap-3 text-xs uppercase tracking-wide text-surface-600-400">
                    <span>
                      {#if mediaTranscodeProgress?.stage === "uploading"}
                        Upload
                      {:else if mediaTranscodeProgress?.hardwareMode === "vaapi"}
                        VA
                      {:else if mediaTranscodeProgress?.hardwareMode === "cuda"}
                        CUDA
                      {:else if mediaTranscodeProgress?.hardwareMode === "software"}
                        Software
                      {:else}
                        Detecting...
                      {/if}
                    </span>
                    <span>{Math.round(mediaTranscodeProgress?.progress ?? 0)}%</span>
                  </div>
                  <div class="h-2 overflow-hidden rounded-full bg-surface-300-700">
                    <div
                      class="h-full rounded-full bg-primary-500 transition-[width] duration-200"
                      style={`width: ${Math.max(4, mediaTranscodeProgress?.progress ?? 0)}%`}
                    ></div>
                  </div>
                  <p class="text-sm text-surface-600-400">
                    {#if mediaTranscodeProgress?.stage === "transcoding"}
                      {pendingMedia.messageType === "m.video" ? "Transcoding video..." : "Transcoding image..."}
                    {:else if mediaTranscodeProgress?.stage === "uploading"}
                      Uploading media...
                    {:else if mediaTranscodeProgress?.stage === "finalizing"}
                      Finalizing the output...
                    {:else if mediaTranscodeProgress?.stage === "cancelled"}
                      Cancelling transcode...
                    {:else}
                      Preparing media...
                    {/if}
                  </p>
                </div>
              {/if}

              {#if mediaErrorMessage}
                <div class="rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-700">
                  {mediaErrorMessage}
                  {#if pendingMedia && pendingMedia.limitError}
                    <button type="button" class="ml-2 underline" onclick={() => pendingMedia && void validatePendingMedia(pendingMedia.filePath)}>Retry limit check</button>
                  {/if}
                </div>
              {/if}

            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}
