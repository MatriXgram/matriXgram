import { invokeMatrixCommand } from "../command-client";
import { normalizeChatSummaryImageUrl, normalizeImageUrl, normalizeMessageImageUrl } from "./media";
import type {
  MatrixChatMessage,
  MatrixChatSummary,
  MatrixGetEmojiPacksResponse,
  MatrixGetUploadLimitResponse,
  MatrixGetChatNavigationRequest,
  MatrixGetChatNavigationResponse,
  MatrixGetChatsResponse,
  MatrixGetRoomImageResponse,
  MatrixGetUserAvatarResponse,
  MatrixGetChatMessagesRequest,
  MatrixGetChatMessagesResponse,
  MatrixGetThreadMessagesRequest,
  MatrixGetRoomMembersRequest,
  MatrixRoomMember,
  MatrixPickerCustomEmoji,
  MatrixSendChatMessageRequest,
  MatrixSendChatMessageResponse,
  MatrixCancelMediaTranscodeRequest,
  MatrixCancelMediaTranscodeResponse,
  MatrixSendMediaFileRequest,
  MatrixSendMediaFileResponse,
  MatrixStreamChatMessagesRequest,
  MatrixStreamChatMessagesResponse,
  MatrixToggleReactionRequest,
  MatrixToggleReactionResponse,
  MatrixRedactMessageRequest,
  MatrixRedactMessageResponse,
  MatrixGetPinnedEventsRequest,
  MatrixGetPinnedEventsResponse,
  MatrixSetMessagePinnedRequest,
  MatrixSetMessagePinnedResponse,
  MatrixSetRootSpaceOrderRequest,
  MatrixSetRootSpaceOrderResponse,
  MatrixCopyImageToClipboardRequest,
  MatrixResolveVideoUrlRequest,
  MatrixResolveVideoUrlResponse,
  MatrixDownloadFileRequest,
  MatrixDownloadFileResponse,
  MatrixGetUserDevicesResponse,
  MatrixOwnVerificationStatus,
  MatrixRequestVerificationResponse,
  MatrixVerificationFlowResponse,
  MatrixTriggerRoomUpdateRequest,
  MatrixTriggerRoomUpdateResponse,
} from "./types";

export async function matrixGetRoomMembers(roomId: string): Promise<MatrixRoomMember[]> {
  return invokeMatrixCommand<MatrixRoomMember[]>("matrix_get_room_members", {
    request: { roomId } satisfies MatrixGetRoomMembersRequest,
  });
}

type PickerAssets = {
  customEmoji: MatrixPickerCustomEmoji[];
};

export async function matrixGetPickerAssets(): Promise<PickerAssets> {
  const response = await invokeMatrixCommand<MatrixGetEmojiPacksResponse>("matrix_get_emoji_packs");

  const customEmoji = response.customEmoji.flatMap((emoji: MatrixPickerCustomEmoji) => {
    const normalizedUrl = normalizeImageUrl(emoji.url);
    if (!normalizedUrl) {
      return [];
    }

    return [{
      ...emoji,
      url: normalizedUrl,
      category: emoji.category ?? undefined,
    }];
  });

  return {
    customEmoji,
  };
}

export async function matrixGetChats(): Promise<MatrixChatSummary[]> {
  const response = await invokeMatrixCommand<MatrixGetChatsResponse>("matrix_get_chats");
  return response.chats.map(normalizeChatSummaryImageUrl);
}

export async function matrixGetRoomImage(roomId: string): Promise<string | null> {
  const response = await invokeMatrixCommand<MatrixGetRoomImageResponse>("matrix_get_room_image", {
    request: { roomId },
  });

  return normalizeImageUrl(response.imageUrl);
}

export async function matrixGetUserAvatar(roomId: string, userId: string): Promise<string | null> {
  const response = await invokeMatrixCommand<MatrixGetUserAvatarResponse>("matrix_get_user_avatar", {
    request: { roomId, userId },
  });

  return normalizeImageUrl(response.imageUrl);
}

const mediaResolveRequests = new Map<string, Promise<string | null>>();
export async function matrixResolveMediaUrl(mediaUrl: string): Promise<string | null> {
  if (!mediaUrl.startsWith("mxc://")) return normalizeImageUrl(mediaUrl);
  const pending = mediaResolveRequests.get(mediaUrl);
  if (pending) return pending;
  const request = invokeMatrixCommand<import("./types").MatrixResolveMediaUrlResponse>(
    "matrix_resolve_media_url", { request: { mediaUrl } },
  ).then((response) => normalizeImageUrl(response.imageUrl)).finally(() => {
    mediaResolveRequests.delete(mediaUrl);
  });
  mediaResolveRequests.set(mediaUrl, request);
  return request;
}

export async function matrixGetChatNavigation(
  input?: MatrixGetChatNavigationRequest,
): Promise<MatrixGetChatNavigationResponse> {
  const response = await invokeMatrixCommand<MatrixGetChatNavigationResponse>("matrix_get_chat_navigation", {
    request: input ?? null,
  });

  return {
    ...response,
    rootSpaces: response.rootSpaces.map(normalizeChatSummaryImageUrl),
    rootScopedRooms: response.rootScopedRooms.map(normalizeChatSummaryImageUrl),
  };
}

export async function matrixSetRootSpaceOrder(
  input: MatrixSetRootSpaceOrderRequest,
): Promise<MatrixSetRootSpaceOrderResponse> {
  return invokeMatrixCommand<MatrixSetRootSpaceOrderResponse>("matrix_set_root_space_order", {
    request: input,
  });
}

export async function matrixGetChatMessages(
  input: MatrixGetChatMessagesRequest,
): Promise<MatrixGetChatMessagesResponse> {
  const response = await invokeMatrixCommand<MatrixGetChatMessagesResponse>("matrix_get_chat_messages", {
    request: input,
  });

  return {
    ...response,
    messages: response.messages.map(normalizeMessageImageUrl),
  };
}

export async function matrixGetThreadMessages(
  input: MatrixGetThreadMessagesRequest,
): Promise<MatrixChatMessage[]> {
  const messages = await invokeMatrixCommand<MatrixChatMessage[]>("matrix_get_thread_messages", {
    request: input,
  });
  return messages.map(normalizeMessageImageUrl);
}

export async function matrixStreamChatMessages(
  input: MatrixStreamChatMessagesRequest,
): Promise<MatrixStreamChatMessagesResponse> {
  return invokeMatrixCommand<MatrixStreamChatMessagesResponse>("matrix_stream_chat_messages", {
    request: input,
  });
}

export async function matrixSendChatMessage(
  input: MatrixSendChatMessageRequest,
): Promise<MatrixSendChatMessageResponse> {
  return invokeMatrixCommand<MatrixSendChatMessageResponse>("matrix_send_chat_message", {
    request: input,
  });
}

export async function matrixSendMediaFile(
  input: MatrixSendMediaFileRequest,
): Promise<MatrixSendMediaFileResponse> {
  return invokeMatrixCommand<MatrixSendMediaFileResponse>("matrix_send_media_file", {
    request: input,
  });
}

export async function matrixGetFileSize(filePath: string): Promise<number> {
  return invokeMatrixCommand<number>("matrix_get_file_size", { filePath });
}

export async function matrixGetUploadLimit(): Promise<number> {
  const response = await invokeMatrixCommand<MatrixGetUploadLimitResponse>("matrix_get_upload_limit");
  return response.maxUploadSize;
}

export async function matrixSavePastedFile(fileName: string, bytes: Uint8Array): Promise<string> {
  return invokeMatrixCommand<string>("matrix_save_pasted_file", {
    fileName,
    bytes: Array.from(bytes),
  });
}

export async function matrixRemovePastedFile(filePath: string): Promise<void> {
  return invokeMatrixCommand<void>("matrix_remove_pasted_file", { filePath });
}

export async function matrixCancelMediaTranscode(
  input: MatrixCancelMediaTranscodeRequest,
): Promise<MatrixCancelMediaTranscodeResponse> {
  return invokeMatrixCommand<MatrixCancelMediaTranscodeResponse>("matrix_cancel_media_transcode", {
    request: input,
  });
}

export async function matrixToggleReaction(
  input: MatrixToggleReactionRequest,
): Promise<MatrixToggleReactionResponse> {
  return invokeMatrixCommand<MatrixToggleReactionResponse>("matrix_toggle_reaction", {
    request: input,
  });
}

export async function matrixRedactMessage(
  input: MatrixRedactMessageRequest,
): Promise<MatrixRedactMessageResponse> {
  return invokeMatrixCommand<MatrixRedactMessageResponse>("matrix_redact_message", {
    request: input,
  });
}

const pinnedEventsCache = new Map<string, { promise: Promise<MatrixGetPinnedEventsResponse>; expiresAt: number }>();
export async function matrixGetPinnedEvents(
  input: MatrixGetPinnedEventsRequest,
): Promise<MatrixGetPinnedEventsResponse> {
  const cached = pinnedEventsCache.get(input.roomId);
  if (cached && cached.expiresAt > Date.now()) return cached.promise;
  const promise = invokeMatrixCommand<MatrixGetPinnedEventsResponse>("matrix_get_pinned_events", {
    request: input,
  });
  pinnedEventsCache.set(input.roomId, { promise, expiresAt: Date.now() + 60_000 });
  void promise.catch(() => pinnedEventsCache.delete(input.roomId));
  return promise;
}

export async function matrixPrefetchPinnedMessageContexts(roomIds: string[], concurrency = 3): Promise<void> {
  const roomQueue = [...new Set(roomIds)];
  const contextQueue: Array<{ roomId: string; eventId: string }> = [];
  let roomIndex = 0;
  const roomWorkers = Array.from({ length: Math.min(concurrency, roomQueue.length) }, async () => {
    while (roomIndex < roomQueue.length) {
      const roomId = roomQueue[roomIndex++];
      if (!roomId) continue;
      try {
        const { eventIds } = await matrixGetPinnedEvents({ roomId });
        contextQueue.push(...eventIds.map((eventId) => ({ roomId, eventId })));
      } catch {
        // Continue prefetching other rooms if one homeserver does not return pins.
      }
    }
  });
  await Promise.all(roomWorkers);

  let contextIndex = 0;
  const contextWorkers = Array.from({ length: Math.min(concurrency, contextQueue.length) }, async () => {
    while (contextIndex < contextQueue.length) {
      const item = contextQueue[contextIndex++];
      if (!item) continue;
      try {
        await matrixGetReadContext(item.roomId, item.eventId);
      } catch {
        // A later jump retries the context request if background prefetch failed.
      }
    }
  });
  await Promise.all(contextWorkers);
}

export async function matrixSetMessagePinned(
  input: MatrixSetMessagePinnedRequest,
): Promise<MatrixSetMessagePinnedResponse> {
  const response = await invokeMatrixCommand<MatrixSetMessagePinnedResponse>("matrix_set_message_pinned", {
    request: input,
  });
  pinnedEventsCache.delete(input.roomId);
  return response;
}

export async function matrixCopyImageToClipboard(
  input: MatrixCopyImageToClipboardRequest,
): Promise<void> {
  return invokeMatrixCommand<void>("matrix_copy_image_to_clipboard", {
    request: input,
  });
}

export async function matrixReadClipboardText(): Promise<string> {
  return invokeMatrixCommand<string>("matrix_read_clipboard_text");
}

export async function matrixResolveVideoUrl(
  input: MatrixResolveVideoUrlRequest,
): Promise<MatrixResolveVideoUrlResponse> {
  return invokeMatrixCommand<MatrixResolveVideoUrlResponse>("matrix_resolve_video_url", {
    request: input,
  });
}

export async function matrixDownloadFile(
  input: MatrixDownloadFileRequest,
): Promise<MatrixDownloadFileResponse> {
  return invokeMatrixCommand<MatrixDownloadFileResponse>("matrix_download_file", {
    request: input,
  });
}

export async function matrixTriggerRoomUpdate(
  input?: MatrixTriggerRoomUpdateRequest,
): Promise<MatrixTriggerRoomUpdateResponse> {
  return invokeMatrixCommand<MatrixTriggerRoomUpdateResponse>("matrix_trigger_room_update", {
    request: input ?? null,
  });
}

export async function matrixOwnVerificationStatus(): Promise<MatrixOwnVerificationStatus> {
  return invokeMatrixCommand<MatrixOwnVerificationStatus>("matrix_own_verification_status");
}

export async function matrixGetUserDevices(userId: string): Promise<MatrixGetUserDevicesResponse> {
  return invokeMatrixCommand<MatrixGetUserDevicesResponse>("matrix_get_user_devices", {
    request: { userId },
  });
}

export async function matrixRequestDeviceVerification(
  userId: string,
  deviceId: string,
): Promise<MatrixRequestVerificationResponse> {
  return invokeMatrixCommand<MatrixRequestVerificationResponse>(
    "matrix_request_device_verification",
    { request: { userId, deviceId } },
  );
}

export async function matrixGetVerificationFlow(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_get_verification_flow", {
    request: { userId, flowId },
  });
}

export async function matrixAcceptVerificationRequest(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_accept_verification_request", {
    request: { userId, flowId },
  });
}

export async function matrixStartSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_start_sas_verification", {
    request: { userId, flowId },
  });
}

export async function matrixAcceptSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_accept_sas_verification", {
    request: { userId, flowId },
  });
}

export async function matrixConfirmSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_confirm_sas_verification", {
    request: { userId, flowId },
  });
}

export async function matrixJoinRoom(
  roomIdOrAlias: string,
  serverNames?: string[],
): Promise<{ roomId: string }> {
  return await invokeMatrixCommand<{ roomId: string }>("matrix_join_room", { request: { roomIdOrAlias, serverNames }});
}

type PublicRoomsResult = import("./types").MatrixSearchPublicRoomsResponse;
const publicRoomsCache = new Map<string, PublicRoomsResult>();
const publicRoomsRequests = new Map<string, Promise<PublicRoomsResult>>();

function publicRoomsKey(serverName: string, searchTerm = "") {
  return `${serverName.trim().toLowerCase()}\n${searchTerm.trim().toLowerCase()}`;
}

export function getCachedPublicRooms(serverName: string, searchTerm = "") {
  return publicRoomsCache.get(publicRoomsKey(serverName, searchTerm)) ?? null;
}

export async function matrixSearchPublicRooms(input: import("./types").MatrixSearchPublicRoomsRequest) {
  const key = publicRoomsKey(input.serverName, input.searchTerm ?? "");
  const requestKey = `${key}\n${input.since ?? ""}`;
  const pending = publicRoomsRequests.get(requestKey);
  if (pending) return pending;

  const request = invokeMatrixCommand<PublicRoomsResult>("matrix_search_public_rooms", { request: input })
    .then(async (response) => ({
      ...response,
      rooms: await Promise.all(response.rooms.map(async (room) => ({
        ...room,
        avatarUrl: await resolvePublicRoomAvatar(room.avatarUrl),
      }))),
    }))
    .then((response) => {
      if (!input.since) publicRoomsCache.set(key, response);
      return response;
    })
    .finally(() => publicRoomsRequests.delete(requestKey));

  publicRoomsRequests.set(requestKey, request);
  return request;
}

export function primeMatrixPublicRooms(serverName: string) {
  if (!serverName || getCachedPublicRooms(serverName)) return;
  void matrixSearchPublicRooms({ serverName, limit: 20 }).catch(() => {});
}

async function resolvePublicRoomAvatar(imageUrl: string | null | undefined): Promise<string | null> {
  if (!imageUrl?.startsWith("mxc://")) return normalizeImageUrl(imageUrl);
  return matrixResolveMediaUrl(imageUrl).catch(() => null);
}


export async function matrixGetUserProfile(roomId: string, userId: string) {
  const profile = await invokeMatrixCommand<import("./types").MatrixUserProfileResponse>(
    "matrix_get_user_profile", { request: { roomId, userId } },
  );
  return { ...profile, avatarUrl: normalizeImageUrl(profile.avatarUrl) };
}

export async function matrixOpenDirectChat(userId: string, encrypted = true) {
  return invokeMatrixCommand<import("./types").MatrixOpenDirectChatResponse>(
    "matrix_open_direct_chat", { request: { userId, encrypted } },
  );
}

export function matrixGetRoomReadState(roomId: string) {
  return invokeMatrixCommand<import("./types").MatrixRoomReadState>("matrix_get_room_read_state", { request: { roomId } });
}
export function matrixMarkRoomRead(request: import("./types").MatrixMarkReadRequest) {
  return invokeMatrixCommand<void>("matrix_mark_room_read", { request });
}
const readContextCache = new Map<string, { promise: Promise<import("./types").MatrixReadContextResponse>; expiresAt: number }>();
export async function matrixGetReadContext(roomId: string, eventId: string) {
  const key = `${roomId}\n${eventId}`;
  const cached = readContextCache.get(key);
  if (cached && cached.expiresAt > Date.now()) return cached.promise;

  const promise = invokeMatrixCommand<import("./types").MatrixReadContextResponse>("matrix_get_read_context", { request: { roomId, eventId } })
    .then((result) => ({ ...result, messages: result.messages.map(normalizeMessageImageUrl) }));
  readContextCache.set(key, { promise, expiresAt: Date.now() + 60_000 });
  while (readContextCache.size > 128) {
    const oldest = readContextCache.keys().next().value;
    if (oldest === undefined) break;
    readContextCache.delete(oldest);
  }
  void promise.catch(() => readContextCache.delete(key));
  return promise;
}
const replyEventCache = new Map<string, Promise<MatrixChatMessage>>();
export async function matrixGetReplyEvent(roomId: string, eventId: string) {
  const key = `${roomId}\n${eventId}`;
  let pending = replyEventCache.get(key);
  if (!pending) {
    pending = invokeMatrixCommand<MatrixChatMessage>("matrix_get_reply_event", {
      request: { roomId, eventId },
    }).then(normalizeMessageImageUrl).then((message) => {
      if (message.messageType === "m.image" && !message.imageUrl) replyEventCache.delete(key);
      return message;
    });
    replyEventCache.set(key, pending);
    while (replyEventCache.size > 256) {
      const oldest = replyEventCache.keys().next().value;
      if (oldest === undefined) break;
      replyEventCache.delete(oldest);
    }
    void pending.catch(() => replyEventCache.delete(key));
  }
  return pending;
}
export async function matrixGetNewerMessages(roomId: string, from: string) {
  const result = await invokeMatrixCommand<import("./types").MatrixReadContextResponse>("matrix_get_newer_messages", { request: { roomId, from, limit: 50 } });
  return { ...result, messages: result.messages.map(normalizeMessageImageUrl) };
}
