export { default as RoomAvatar } from "./RoomAvatar.svelte";
export { default as RoomSettingsDialog } from "./RoomSettingsDialog.svelte";
export {
  roomImageCache,
  senderAvatarCache,
  avatarChangeEpoch,
  roomImageChangeEpoch,
  clearImageCaches,
  invalidateSenderAvatar,
} from "./image-cache.svelte";
export {
  VIRTUAL_DMS_ROOT_ID,
  VIRTUAL_UNSPACED_ROOT_ID,
  isVirtualRoomId,
} from "./constants";
