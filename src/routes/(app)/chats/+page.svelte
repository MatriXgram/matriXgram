<script lang="ts">
  import { page } from "$app/state";
  import CommunityExplorer from "$lib/components/navigation/rooms/CommunityExplorer.svelte";
  import { matrixGetRoomReadState, matrixMarkRoomRead, matrixGetReadContext, matrixGetNewerMessages } from "$lib/chats/api";
  import { firstUnreadIndex } from "$lib/chats/read-position";
import { formatDraft } from "$lib/components/messaging/composer/draftFormatting";
import type { ComposerMention } from "$lib/components/messaging/composer";
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";

  import {
    matrixCancelMediaTranscode,
    matrixSendChatMessage,
    matrixSendMediaFile,
    matrixStreamChatMessages,
    matrixToggleReaction,
  } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
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
  import { stripMxReplyBlock } from "$lib/components/messaging/shared";

  let loadingMessages = $state(false);
  let errorMessage = $state("");
  let composerErrorMessage = $state("");
  let messageDraft = $state("");
  let sendingMessage = $state(false);
  let replyToMessage = $state<TimelineMessage | null>(null);
  let composerFocusNonce = $state(0);
  let roomSettingsDialogRoomId = $state<string | null>(null);
  let pendingMedia = $state<{
    filePath: string;
    fileName: string;
    messageType: "m.image" | "m.video" | "m.file";
    compressMedia: boolean;
  } | null>(null);
  let mediaCaption = $state("");
  let mediaCaptionInput = $state<HTMLTextAreaElement | null>(null);
  let mediaEmojiPickerOpen = $state(false);
  let sendingMedia = $state(false);
  let mediaErrorMessage = $state("");
  let activeMediaFilePath = $state("");
  let mediaTranscodeProgress = $state<MatrixMediaTranscodeProgressEvent | null>(null);
  let forwardingMessage = $state<TimelineMessage | null>(null);
  let forwardError = $state("");
  let forwarding = $state(false);

  let messages = $state<TimelineMessage[]>([]);
  let nextFrom = $state<string | null>(null);
  let timelineElement = $state<HTMLElement | null>(null);
  let activeStreamId = $state("");
  let activeLoadKind = $state<MatrixMessageLoadKind | null>(null);
  let streamMessageCount = $state(0);

  let unreadBoundaryId = $state<string | null>(null);
  let unreadRemaining = $state(0);
  let unreadTotal = 0;
  let initialLatestTimestamp = 0;
  let readMarkerId: string | null = null;
  let readPositionReady = $state(false);
  let initialPositionPending = $state(true);
  let positioningInitial = false;
  let nextTo = $state<string | null>(null);
  let loadingNewer = $state(false);
  let readingContext = false;
  let jumpRequested = false;
  let highestReadTimestamp = 0;
  const consumedUnread = new Set<string>();
  const newUnread = new Set<string>();
  let readTimer: ReturnType<typeof setTimeout>;
  let readRequestInFlight = false;

  const seenEventIds = new Set<string>();
  const reactionStateOverrides = new Map<
    string,
    Map<string, { added: boolean; timeout: ReturnType<typeof setTimeout>; token: symbol }>
  >();

  const AUTO_LOAD_TOP_THRESHOLD_PX = 600;
  const NEAR_BOTTOM_PX = 48;
  const PROGRAMMATIC_SCROLL_SETTLE_MS = 800;
  // The room worker holds the per-homeserver sync lock for as long as the
  // server keeps a long poll open, and a send queues behind it, so a slow
  // homeserver can spend this long before the first page lands.
  const MESSAGE_LOAD_TIMEOUT_MS = 30000;

  let pendingPinToBottomRoomId = "";
  let pendingRoomMessages: MatrixSelectedRoomMessagesEvent | null = null;

  let previousSelectedRoomId = "";
  let lastScrollTop = 0;
  let restoringTimelinePosition = false;
  let isScrolledToBottom = $state(true);
  let programmaticScrollUntil = 0;

  onMount(() => {
    let unlisten = () => {};
    let unlistenTranscode = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: () => {},
        onRoomUpdated: () => {},
        onRoomRemoved: () => {},
        onSelectedRoomMessages: applySelectedRoomMessages,
        onChatMessagesStream: applyChatMessageStream,
      });

      unlistenTranscode = await listen<MatrixMediaTranscodeProgressEvent>(
        EVENT_MEDIA_TRANSCODE_PROGRESS,
        (event) => {
          if (event.payload.filePath !== activeMediaFilePath) {
            return;
          }

          mediaTranscodeProgress = event.payload;
        },
      );
    })();

    return () => {
      clearTimeout(readTimer);
      unlisten();
      unlistenTranscode();
    };
  });

  $effect(() => {
    const selectedRoomId = $shellSelectedRoomId;

    if (!selectedRoomId) {
      previousSelectedRoomId = "";
      loadingMessages = false;
      resetRoomState();
      return;
    }

    if (selectedRoomId === previousSelectedRoomId) {
      return;
    }

    previousSelectedRoomId = selectedRoomId;
    resetRoomState();

    void loadMessages(selectedRoomId);
  });

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
      nextFrom = payload.nextFrom ?? null;
      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;

      // A room update that arrived while the timeline was streaming is
      // applied now that the stream is done.
      flushPendingRoomMessages();

  // A fresh room always lands on the newest message, wherever the previous
  // room was scrolled to.
  if (payload.loadKind === "initial") {
    queuePinTimelineToBottom(payload.roomId);
  }
      return;
    }

    if (payload.messages) {
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
      // Older pagination keeps newest->older stream order; prepend preserves chronology.
      const olderMessage = payload.message;
      preserveReadingPosition(() => { messages = [olderMessage, ...messages]; });
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
    if (loadingMessages || !pendingRoomMessages) {
      return;
    }

    const payload = pendingRoomMessages;
    pendingRoomMessages = null;
    applySelectedRoomMessagesNow(payload);
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

  function createPendingMedia(filePath: string) {
    const messageType = detectMediaKind(filePath);
    pendingMedia = {
      filePath,
      fileName: filePath.split(/[\\/]/).pop() || "attachment",
      messageType,
      compressMedia: messageType !== "m.video",
    };
    mediaCaption = "";
    mediaEmojiPickerOpen = false;
    mediaErrorMessage = "";
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
    if (!pendingMedia) {
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

  async function sendPendingMedia() {
    if (!pendingMedia || sendingMedia || !$shellSelectedRoomId) {
      return;
    }

    const mediaDraft = pendingMedia;
    activeMediaFilePath = mediaDraft.filePath;
    mediaTranscodeProgress = {
      roomId: $shellSelectedRoomId,
      filePath: mediaDraft.filePath,
      stage: mediaDraft.compressMedia ? "preparing" : "uploading",
      progress: 0,
      hardwareMode: "",
    };
    const optimistic = buildOptimisticMediaMessage();
    const localId = optimistic.localId;

    if (!localId) {
      return;
    }

    messages = [...messages, optimistic];
    queuePinTimelineToBottom($shellSelectedRoomId, { force: true });

    sendingMedia = true;
    mediaErrorMessage = "";

    try {
      await matrixSendMediaFile({
        roomId: $shellSelectedRoomId,
        filePath: mediaDraft.filePath,
        compressMedia: mediaDraft.compressMedia,
        caption: mediaCaption.trim() || null,
      });

      pendingMedia = null;
      mediaCaption = "";
      mediaEmojiPickerOpen = false;
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
        messages = messages.filter((message) => message.localId !== localId);
      } else {
        mediaErrorMessage = errorText;
        messages = messages.map((message) => {
          if (message.localId !== localId) {
            return message;
          }

          return {
            ...message,
            sendState: "failed",
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

    composerErrorMessage = "";

    messages = messages.map((timelineMessage) => {
      if (timelineMessage.localId !== message.localId) {
        return timelineMessage;
      }

      return {
        ...timelineMessage,
        sendState: "sending",
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

  function handleReplyToMessage(message: TimelineMessage) {
    if (!message.eventId) {
      return;
    }

    replyToMessage = message;
    composerFocusNonce += 1;
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
    clearTimeout(readTimer);
    unreadBoundaryId = null; unreadRemaining = 0; unreadTotal = 0;
    readMarkerId = null; initialLatestTimestamp = 0;
    consumedUnread.clear(); newUnread.clear();
    readPositionReady = false; initialPositionPending = true;
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
    messages = [];
    nextFrom = null;
  }

  async function loadMessages(roomId: string, jumpToEnd = false) {
    loadingMessages = true;
    errorMessage = "";
    seenEventIds.clear();

    const streamId = createStreamId();
    activeStreamId = streamId;
    activeLoadKind = "initial";
    streamMessageCount = 0;

    // Defense-in-depth: if the backend never emits a terminal `done` event
    // (e.g. a stream failure), clear the loading state after a timeout so the
    // UI never hangs in an infinite loading loop.
    const timeoutHandle = setTimeout(() => {
      if (activeStreamId !== streamId) {
        return;
      }

      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      errorMessage = "Timed out loading messages";
    }, MESSAGE_LOAD_TIMEOUT_MS);

    try {
      if (!jumpToEnd) {
        try {
          const read = await matrixGetRoomReadState(roomId);
          if ($shellSelectedRoomId !== roomId || activeStreamId !== streamId) return;
          readMarkerId = read.eventId ?? null;
          unreadTotal = read.unreadCount;
          unreadRemaining = unreadTotal;
          initialLatestTimestamp = read.latestTimestamp ?? Date.now();
          if (readMarkerId && unreadTotal > 0) {
            try {
              const context = await matrixGetReadContext(roomId, readMarkerId);
              if ($shellSelectedRoomId !== roomId || activeStreamId !== streamId) return;
              if (context.messages.length) {
                messages = context.messages;
                for (const message of messages) if (message.eventId) seenEventIds.add(message.eventId);
                nextFrom = context.nextFrom ?? null; nextTo = context.nextTo ?? null;
                readingContext = Boolean(nextTo);
                activeStreamId = ""; activeLoadKind = null; loadingMessages = false;
                readPositionReady = true;
                clearTimeout(timeoutHandle);
                void positionInitialRead();
                return;
              }
            } catch { /* A redacted/unavailable marker falls back to the newest page. */ }
          }
        } catch { initialLatestTimestamp = Date.now(); }
      }
      readPositionReady = true;
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
      if (message.eventId && message.sender !== $shellCurrentUserId &&
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
      const divider = timelineElement.querySelector<HTMLElement>("[data-unread-divider]");
      if (divider) {
        timelineElement.scrollTop += divider.getBoundingClientRect().top - timelineElement.getBoundingClientRect().top - 16;
      } else {
        timelineElement.scrollTop = timelineElement.scrollHeight;
      }
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
    if (!element || !roomId || !readPositionReady || initialPositionPending || restoringTimelinePosition ||
        document.hidden || !document.hasFocus() || readRequestInFlight) return;
    const viewport = element.getBoundingClientRect();
    const visible = Array.from(element.querySelectorAll<HTMLElement>("[data-message-event-id]"))
      .filter(node => { const rect = node.getBoundingClientRect(); return rect.bottom <= viewport.bottom + 2 && rect.bottom > viewport.top; });
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
      if (item.eventId && item.sender !== $shellCurrentUserId) consumedUnread.add(item.eventId);
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

  async function loadJumpContext(eventId: string): Promise<boolean> {
    const roomId = $shellSelectedRoomId;
    if (!roomId) return false;
    if (messages.some((message) => message.eventId === eventId)) return true;

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
</script>
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
      showJumpToLatest={!initialPositionPending && (!isScrolledToBottom || Boolean(nextTo) || unreadRemaining > 0)}
      onJumpToLatest={jumpToLatest}
      currentUserId={$shellCurrentUserId || null}
      pickerCustomEmoji={$shellPickerCustomEmoji}
      roomEncrypted={selectedRoomEncrypted()}
      roomName={$shellChats.find((chat) => chat.roomId === $shellSelectedRoomId)?.displayName ?? ""}
      showSenderNames={selectedRoomIsGroup()}
      {loadingMessages}
      {activeLoadKind}
      {streamMessageCount}
      error={errorMessage}
      isSending={sendingMessage}
      onTimelineElementChange={(element) => timelineElement = element}
      onScroll={handleTimelineScroll}
      onProgrammaticScroll={beginProgrammaticScroll}
      onOpenRoomSettings={(roomIdToOpen) => (roomSettingsDialogRoomId = roomIdToOpen)}
      onRetryMessage={retryMessage}
      onToggleReaction={handleToggleReaction}
      onReplyToMessage={handleReplyToMessage}
      onForwardMessage={handleForwardMessage}
      onLoadJumpContext={loadJumpContext}
    />

    <div class="composer-dock">
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
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
        role="button"
        tabindex="0"
        onclick={handlePendingMediaBackdropClick}
        onkeydown={handlePendingMediaBackdropKeydown}
      >
        <div class="max-h-[92vh] w-full overflow-y-auto rounded-2xl border border-surface-300-700 bg-surface-100-900 shadow-2xl {pendingMedia.messageType === 'm.image' ? 'max-w-md' : 'max-w-3xl'}">
          <div class="flex items-center justify-between border-b border-surface-300-700 {pendingMedia.messageType === 'm.image' ? 'px-3 py-2' : 'px-5 py-4'}">
            {#if pendingMedia.messageType === "m.image"}
              <p class="text-sm font-medium text-surface-700-300">Send image</p>
            {:else}
              <div>
                <p class="text-lg font-semibold text-surface-900-100">Send file</p>
                <p class="text-sm text-surface-600-400">{pendingMedia.fileName}</p>
              </div>
            {/if}
            <button class="btn preset-outlined" type="button" onclick={cancelPendingMedia}>
              {pendingMedia.messageType === "m.image" ? "×" : "Cancel"}
            </button>
          </div>

          <div class={pendingMedia.messageType === "m.image" ? "flex flex-col gap-2 p-3" : "grid gap-4 p-4 md:grid-cols-[minmax(0,1.25fr)_minmax(16rem,1fr)]"}>
            <div class={pendingMedia.messageType === "m.image" ? "flex h-44 items-center justify-center rounded-xl bg-surface-200-800 p-2" : "flex min-h-[12rem] items-center justify-center rounded-xl bg-surface-200-800 p-3"}>
              {#if pendingMedia.messageType === "m.image"}
                <img
                  src={convertFileSrc(pendingMedia.filePath)}
                  alt={pendingMedia.fileName}
                  class="max-h-40 max-w-full rounded-lg object-contain"
                />
              {:else if pendingMedia.messageType === "m.video"}
                <!-- svelte-ignore a11y_media_has_caption -->
                <video
                  src={convertFileSrc(pendingMedia.filePath)}
                  controls
                  playsinline
                  class="max-h-[38vh] w-full rounded-lg bg-black"
                ></video>
              {:else}
                <div class="text-center">
                  <div class="text-5xl">Attachment</div>
                  <p class="mt-3 text-lg font-medium">{pendingMedia.fileName}</p>
                </div>
              {/if}
            </div>

            <div class={pendingMedia.messageType === "m.image" ? "space-y-2" : "space-y-3 rounded-xl border border-surface-300-700 bg-surface-100-900 p-3"}>
              {#if pendingMedia.messageType !== "m.file"}
                <label class={pendingMedia.messageType === "m.image" ? "flex items-center gap-2 px-1 text-sm" : "flex items-start gap-3 rounded-lg border border-surface-300-700 p-3"}>
                  <input type="checkbox" bind:checked={pendingMedia.compressMedia} class="mt-0.5" />
                  <span class="font-medium">Compress before sending</span>
                  {#if pendingMedia.messageType !== "m.image"}
                    <span class="block text-sm text-surface-600-400">
                      Videos become VP9 WebM.
                    </span>
                  {/if}
                </label>
              {/if}
              {#if pendingMedia.messageType !== "m.file"}
                <label class="block space-y-1.5">
                  <div class="relative">
                    <textarea
                      bind:this={mediaCaptionInput}
                      bind:value={mediaCaption}
                      rows="2"
                      maxlength="2000"
                      placeholder="Add a caption..."
                      aria-label="Caption"
                      class="w-full resize-y rounded-lg border border-surface-300-700 bg-surface-200-800 px-3 py-2 pr-12 text-sm text-surface-900-100 outline-none focus:border-primary-500"
                    ></textarea>
                    {#if pendingMedia.messageType === "m.image"}
                      <div class="absolute bottom-1.5 right-1.5">
                        <EmojiPickerPopover
                          bind:open={mediaEmojiPickerOpen}
                          buttonText="☺"
                          buttonClass="grid size-8 place-items-center rounded-md text-lg text-surface-600-400 transition hover:bg-hover hover:text-surface-900-100"
                          popoverClass="rounded-xl border border-hairline bg-panel p-1 shadow-lg"
                          pickerStyle="width: min(280px, calc(100vw - 32px)); height: 260px; --emoji-size: 1.35rem; --category-emoji-size: 1rem; --num-columns: 7;"
                          pickerCustomEmoji={$shellPickerCustomEmoji}
                          onSelect={insertMediaCaptionEmoji}
                        />
                      </div>
                    {/if}
                  </div>
                </label>
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
                </div>
              {/if}

              <button
                type="button"
                class="btn preset-filled w-full"
                disabled={sendingMedia || !$shellSelectedRoomId}
                onclick={sendPendingMedia}
              >
                {#if sendingMedia}Sending...{:else if pendingMedia.messageType === "m.image"}Send image{:else}Send file{/if}
              </button>
            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}
