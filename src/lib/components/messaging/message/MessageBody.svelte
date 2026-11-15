<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { matrixDownloadFile, matrixGetReplyEvent, matrixGetUserAvatar, matrixResolveMediaUrl, matrixResolveVideoUrl } from "$lib/chats/api";
  import { senderAvatarCache } from "$lib/components/navigation/shared/image-cache.svelte";
  import { openUserProfile } from "$lib/chats/user-profile";
  import { userIdFromMatrixLink } from "$lib/chats/matrixLinks";
  import { stripMxReplyBlock, stripPlainReplyFallback } from "../shared";
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    imageMessages: TimelineMessage[];
    roomId: string;
    hasSenderHeader?: boolean;
    onImageContextMenu?: (event: MouseEvent) => void;
  }

  let {
    message,
    imageMessages,
    roomId,
    hasSenderHeader = false,
    onImageContextMenu,
  }: Props = $props();

  let linkContextMenu = $state<{ url: string; x: number; y: number } | null>(null);

  function splitLinks(text: string) {
    const parts: { text: string; url?: string }[] = [];
    const pattern = /https?:\/\/[^\s<>"']+/gi;
    let cursor = 0;
    for (const match of text.matchAll(pattern)) {
      const start = match.index ?? 0;
      const raw = match[0];
      const trailing = raw.match(/[),.!?;:]+$/)?.[0] ?? "";
      const url = raw.slice(0, raw.length - trailing.length);
      if (start > cursor) parts.push({ text: text.slice(cursor, start) });
      if (url) parts.push({ text: url, url });
      if (trailing) parts.push({ text: trailing });
      cursor = start + raw.length;
    }
    if (cursor < text.length) parts.push({ text: text.slice(cursor) });
    return parts;
  }

  function safeExternalUrl(href: string) {
    try {
      const url = new URL(href, window.location.href);
      return url.protocol === "https:" || url.protocol === "http:" ? url.href : null;
    } catch { return null; }
  }

  function showLinkMenu(event: MouseEvent, url: string) {
    event.preventDefault();
    event.stopPropagation();
    linkContextMenu = { url, x: event.clientX, y: event.clientY };
  }

  async function copyContextLink() {
    if (!linkContextMenu) return;
    try { await navigator.clipboard.writeText(linkContextMenu.url); }
    finally { linkContextMenu = null; }
  }

  function imageCaption(body: string | null | undefined): string {
    const caption = body?.trim() ?? "";
    if (!caption) return "";
    const looksLikeImageFilename = /^[^/\\\r\n]{1,180}\.(?:jpe?g|png|gif|webp|avif|bmp|heic|heif|tiff?)$/i.test(caption);
    return looksLikeImageFilename ? "" : caption;
  }

  function mentionLinks(element: HTMLDivElement) {
    const handleClick = (event: MouseEvent) => {
      const link = event.target instanceof Element ? event.target.closest("a[href]") : null;
      if (!(link instanceof HTMLAnchorElement) || !element.contains(link)) return;
      const userId = userIdFromMatrixLink(link.getAttribute("href") ?? "");
      if (userId) {
        event.preventDefault();
        event.stopPropagation();
        openUserProfile({ roomId, userId, displayName: link.textContent?.trim() || null });
        return;
      }
      const url = safeExternalUrl(link.href);
      if (!url) return;
      event.preventDefault();
      event.stopPropagation();
      void openUrl(url);
    };
    const handleContextMenu = (event: MouseEvent) => {
      const link = event.target instanceof Element ? event.target.closest("a[href]") : null;
      if (!(link instanceof HTMLAnchorElement) || !element.contains(link)) return;
      const url = safeExternalUrl(link.href);
      if (url) showLinkMenu(event, url);
    };
    element.addEventListener("click", handleClick);
    element.addEventListener("contextmenu", handleContextMenu);
    return { destroy() { element.removeEventListener("click", handleClick); element.removeEventListener("contextmenu", handleContextMenu); } };
  }

  function mentionAvatars(element: HTMLDivElement) {
    let active = true;
    const avatarImages: HTMLImageElement[] = [];
    const styledLinks: HTMLAnchorElement[] = [];
    for (const link of element.querySelectorAll<HTMLAnchorElement>("a[href]")) {
      const userId = userIdFromMatrixLink(link.getAttribute("href") ?? "");
      if (!userId) continue;
      if (!link.classList.contains("matrix-mention")) {
        link.classList.add("matrix-mention");
        styledLinks.push(link);
      }
      if (link.querySelector(".matrix-mention-avatar")) continue;

      const avatar = document.createElement("img");
      avatar.className = "matrix-mention-avatar";
      avatar.alt = "";
      avatar.setAttribute("aria-hidden", "true");
      avatar.loading = "lazy";
      avatarImages.push(avatar);
      link.prepend(avatar);

      const displayLabel = link.textContent?.trim().replace(/^@/, "") ?? "";
      const initial = Array.from(displayLabel)[0]?.toLocaleUpperCase() || userId.replace(/^@/, "")[0]?.toLocaleUpperCase() || "•";
      const showInitial = () => {
        if (!active || !link.isConnected || !avatar.isConnected) return;
        const fallback = document.createElement("span");
        fallback.className = "matrix-mention-avatar-fallback";
        fallback.textContent = initial;
        fallback.setAttribute("aria-hidden", "true");
        avatar.replaceWith(fallback);
      };
      avatar.addEventListener("error", showInitial, { once: true });

      const cacheKey = `${roomId}\u0000${userId}`;
      void senderAvatarCache.getOrLoad(cacheKey, () => matrixGetUserAvatar(roomId, userId)).then((url) => {
        if (!active || !link.isConnected) return;
        if (url) avatar.src = url;
        else showInitial();
      }).catch(showInitial);
    }

    return {
      destroy() {
        active = false;
        for (const avatar of avatarImages) avatar.remove();
        for (const link of styledLinks) link.classList.remove("matrix-mention");
      },
    };
  }

  function customEmojiFallback(element: HTMLDivElement) {
    let active = true;
    const cleanups: (() => void)[] = [];
    const brokenImage = (image: HTMLImageElement) => {
      if (!image.isConnected || image.dataset.fallbackRendered === "true") return;
      if (!image.hasAttribute("data-mx-emoticon") && !image.hasAttribute("data_mx_emoticon")) return;
      image.dataset.fallbackRendered = "true";
      const label = image.getAttribute("alt")?.trim() || image.getAttribute("title")?.trim() || "Custom emoji";
      const fallback = document.createElement("span");
      fallback.className = "custom-emoji-unavailable";
      const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      icon.setAttribute("viewBox", "0 0 16 16");
      icon.setAttribute("aria-hidden", "true");
      icon.classList.add("custom-emoji-unavailable-icon");
      const frame = document.createElementNS("http://www.w3.org/2000/svg", "rect");
      frame.setAttribute("x", "1.5"); frame.setAttribute("y", "1.5");
      frame.setAttribute("width", "13"); frame.setAttribute("height", "13");
      frame.setAttribute("rx", "1.5");
      const mountain = document.createElementNS("http://www.w3.org/2000/svg", "path");
      mountain.setAttribute("d", "m3.5 11 3-3 2 2 1.5-1.5 2.5 2.5");
      const slash = document.createElementNS("http://www.w3.org/2000/svg", "path");
      slash.setAttribute("d", "m2 2 12 12");
      icon.append(frame, mountain, slash);
      const text = document.createElement("span");
      text.textContent = label;
      fallback.append(icon, text);
      fallback.title = label;
      fallback.setAttribute("role", "img");
      fallback.setAttribute("aria-label", `${label} unavailable`);
      image.replaceWith(fallback);
    };

    for (const image of element.querySelectorAll<HTMLImageElement>("img")) {
      const onError = () => {
        const source = image.getAttribute("src") ?? "";
        const shortcode = (image.getAttribute("alt") ?? "").trim().replace(/^:|:$/g, "").toLocaleLowerCase();
        const storedSource = message.customEmojis.find((emoji) =>
          emoji.shortcode.trim().replace(/^:|:$/g, "").toLocaleLowerCase() === shortcode,
        )?.sourceUrl;
        const mxcSource = source.startsWith("mxc://") ? source : storedSource?.startsWith("mxc://") ? storedSource : null;
        if (mxcSource && image.dataset.authenticatedRetry !== "true") {
          image.dataset.authenticatedRetry = "true";
          image.removeAttribute("src");
          void matrixResolveMediaUrl(mxcSource).then((url) => {
            if (!active || !image.isConnected) return;
            if (url) image.src = url;
            else brokenImage(image);
          }).catch(() => brokenImage(image));
          return;
        }
        brokenImage(image);
      };
      image.addEventListener("error", onError);
      cleanups.push(() => image.removeEventListener("error", onError));
      const source = image.getAttribute("src") ?? "";
      if (source.startsWith("mxc://") || (image.complete && image.naturalWidth === 0)) onError();
    }

    return {
      destroy() {
        active = false;
        for (const cleanup of cleanups) cleanup();
      },
    };
  }

  function codeControls(element: HTMLDivElement) {
    const cleanups: (() => void)[] = [];
    for (const pre of element.querySelectorAll("pre")) {
      const wrapper = document.createElement("div");
      wrapper.className = "code-block";
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = "Copy code";
      button.className = "code-copy";
      button.setAttribute("aria-label", "Copy code");
      let timer: ReturnType<typeof setTimeout>;
      const copy = async () => {
        try {
          await navigator.clipboard.writeText(pre.textContent ?? "");
          button.textContent = "Copied";
        } catch {
          button.textContent = "Copy failed";
        }
        clearTimeout(timer);
        timer = setTimeout(() => { button.textContent = "Copy code"; }, 2000);
      };
      button.addEventListener("click", copy);
      pre.before(wrapper);
      wrapper.append(button, pre);
      cleanups.push(() => { button.removeEventListener("click", copy); clearTimeout(timer); });
    }
    return { destroy() { cleanups.forEach((cleanup) => cleanup()); } };
  }

  let videoElement = $state<HTMLVideoElement | null>(null);
  let videoUrl = $state<string | null>(null);
  let videoError = $state(false);
  let videoLoading = $state(false);
  let videoWaitingForData = $state(false);
  let videoHasFrame = $state(false);

  /// The poster is a still of the first frame. Leaving it on the element once
  /// the video has decoded its own first frame lets the browser keep painting
  /// the still over a video that is already playing, so it is dropped at that
  /// point rather than left to the element.
  const videoPoster = $derived(
    videoHasFrame || !message.thumbnailUrl ? undefined : message.thumbnailUrl,
  );

  let imageError = $state(false);
  let recoveredImageUrl = $state<string | null>(null);
  let imageRecoveryAttempted = false;
  let imageViewerOpen = $state(false);
  let selectedViewerImageIndex = $state(-1);
  let imageCaptionExpanded = $state(false);
  let imageCaptionNeedsExpansion = $state(false);
  let imageCaptionElement = $state<HTMLParagraphElement | null>(null);
  let viewerImageElement = $state<HTMLImageElement | null>(null);
  let viewerScrollElement = $state<HTMLDivElement | null>(null);
  let viewerImageSize = $state<{ width: number; height: number } | null>(null);
  let imageZoom = $state(1);
  let imagePanActive = $state(false);
  let imagePanStart: { x: number; y: number; left: number; top: number } | null = null;
  const activeViewerMessage = $derived(imageMessages[selectedViewerImageIndex] ?? message);
  const activeViewerCaption = $derived(imageCaption(activeViewerMessage.body));
  const displayImageUrl = $derived(recoveredImageUrl ?? message.imageUrl);

  $effect(() => {
    const eventId = message.eventId;
    if (message.messageType !== "m.image" || message.imageUrl || !eventId || imageRecoveryAttempted) return;
    imageRecoveryAttempted = true;
    void matrixGetReplyEvent(roomId, eventId).then((freshMessage) => {
      if (message.eventId === eventId && freshMessage.imageUrl) recoveredImageUrl = freshMessage.imageUrl;
      else if (message.eventId === eventId) imageError = true;
    }).catch(() => {
      if (message.eventId === eventId) imageError = true;
    });
  });

  $effect(() => {
    const element = imageCaptionElement;
    activeViewerCaption;
    if (!element || imageCaptionExpanded) return;
    const updateOverflow = () => {
      imageCaptionNeedsExpansion = element.scrollHeight > element.clientHeight + 1;
    };
    const observer = new ResizeObserver(updateOverflow);
    observer.observe(element);
    requestAnimationFrame(updateOverflow);
    return () => observer.disconnect();
  });

  function openImageViewer() {
    const key = message.eventId ?? message.localId;
    const index = imageMessages.findIndex((item) => (item.eventId ?? item.localId) === key);
    selectedViewerImageIndex = index >= 0 ? index : 0;
    imageZoom = 1;
    viewerImageSize = null;
    imageCaptionExpanded = false;
    imageCaptionNeedsExpansion = false;
    imageViewerOpen = true;
  }

  function navigateViewerImage(direction: -1 | 1) {
    const next = selectedViewerImageIndex + direction;
    if (next < 0 || next >= imageMessages.length) return;
    selectedViewerImageIndex = next;
    imageZoom = 1;
    viewerImageSize = null;
    imageCaptionExpanded = false;
    imageCaptionNeedsExpansion = false;
    imagePanStart = null;
    imagePanActive = false;
    if (viewerScrollElement) {
      viewerScrollElement.scrollLeft = 0;
      viewerScrollElement.scrollTop = 0;
    }
  }

  function fitViewerImage() {
    const image = viewerImageElement;
    if (!image?.naturalWidth || !image.naturalHeight) return;
    const fit = Math.min(
      (window.innerWidth - 16) / image.naturalWidth,
      (window.innerHeight - 16) / image.naturalHeight,
    );
    viewerImageSize = {
      width: Math.max(1, image.naturalWidth * fit),
      height: Math.max(1, image.naturalHeight * fit),
    };
  }

  function changeImageZoom(delta: number) {
    imageZoom = Math.min(4, Math.max(0.25, imageZoom + delta));
  }

  function closeImageViewer() {
    imageViewerOpen = false;
    imageZoom = 1;
    viewerImageSize = null;
  }

  async function retryMessageImage(event: Event) {
    const eventId = message.eventId;
    const image = event.currentTarget;
    if (!(image instanceof HTMLImageElement) || !eventId || imageRecoveryAttempted) {
      imageError = true;
      return;
    }
    imageRecoveryAttempted = true;
    try {
      const freshMessage = await matrixGetReplyEvent(roomId, eventId);
      if (message.eventId !== eventId || !freshMessage.imageUrl) {
        imageError = true;
        return;
      }
      recoveredImageUrl = freshMessage.imageUrl;
      imageError = false;
      // Force WebView to retry even when the recovered cache path is identical
      // to the stale URL that just failed.
      image.removeAttribute("src");
      requestAnimationFrame(() => {
        if (image.isConnected && message.eventId === eventId) image.src = freshMessage.imageUrl!;
      });
    } catch {
      imageError = true;
    }
  }

  function handleImageViewerKeydown(event: KeyboardEvent) {
    if (imageViewerOpen && event.key === "Escape") {
      closeImageViewer();
    } else if (imageViewerOpen && event.key === "ArrowLeft") {
      event.preventDefault();
      navigateViewerImage(-1);
    } else if (imageViewerOpen && event.key === "ArrowRight") {
      event.preventDefault();
      navigateViewerImage(1);
    }
  }

  function handleImageViewerWheel(event: WheelEvent) {
    event.preventDefault();
    changeImageZoom(event.deltaY < 0 ? 0.2 : -0.2);
  }

  function startImagePan(event: PointerEvent) {
    const viewport = viewerScrollElement;
    if (!viewport || event.button !== 0 ||
        (viewport.scrollWidth <= viewport.clientWidth && viewport.scrollHeight <= viewport.clientHeight)) return;
    imagePanStart = {
      x: event.clientX,
      y: event.clientY,
      left: viewport.scrollLeft,
      top: viewport.scrollTop,
    };
    imagePanActive = true;
    viewport.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function moveImagePan(event: PointerEvent) {
    const viewport = viewerScrollElement;
    if (!viewport || !imagePanStart) return;
    viewport.scrollLeft = imagePanStart.left - (event.clientX - imagePanStart.x);
    viewport.scrollTop = imagePanStart.top - (event.clientY - imagePanStart.y);
  }

  function endImagePan(event: PointerEvent) {
    const viewport = viewerScrollElement;
    if (viewport?.hasPointerCapture(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
    imagePanStart = null;
    imagePanActive = false;
  }

  let fileSaved = $state(false);
  let fileError = $state(false);
  let fileLoading = $state(false);

  const renderedFormattedBody = $derived(
    message.formattedBody ? stripMxReplyBlock(message.formattedBody) : null,
  );
  const renderedPlainBody = $derived(stripPlainReplyFallback(message));
  const isUnsupportedMessage = $derived(message.body.startsWith("Unsupported message type:") || message.body === "Unsupported message");

  /// Resolve the video URL on demand when the user starts playback. The video
  /// element is always rendered (with the thumbnail as poster); the API is only
  /// hit once the user presses play.
  ///
  /// Playback is started explicitly once there is something to play. The first
  /// click starts from an element with no `src`, so the browser has nothing to
  /// run and the click is spent on resolving the URL instead.
  async function handleVideoPlay() {
    if (videoUrl || videoLoading || videoError || !message.eventId) {
      return;
    }

    videoLoading = true;
    try {
      const { videoUrl: resolved } = await matrixResolveVideoUrl({
        roomId,
        eventId: message.eventId,
      });
      if (!resolved) {
        videoError = true;
        return;
      }

      videoUrl = resolved;
    } catch {
      videoError = true;
    } finally {
      videoLoading = false;
    }

    await startPlaybackWhenReady();
  }

  /// Play once the element has metadata. Resolves when playback has started or
  /// has failed on its own terms, so a caller never waits on a stalled video.
  async function startPlaybackWhenReady() {
    const element = videoElement;
    if (!element || videoError) {
      return;
    }

    if (element.readyState < HTMLMediaElement.HAVE_METADATA) {
      try {
        await new Promise<void>((resolve, reject) => {
          const cleanup = () => {
            element.removeEventListener("loadedmetadata", handleMetadata);
            element.removeEventListener("canplay", handleCanPlay);
            element.removeEventListener("error", handleError);
          };
          const handleMetadata = () => {
            cleanup();
            resolve();
          };
          const handleCanPlay = () => {
            cleanup();
            resolve();
          };
          const handleError = () => {
            cleanup();
            reject(new Error("Video metadata could not be loaded"));
          };

          element.addEventListener("loadedmetadata", handleMetadata, { once: true });
          element.addEventListener("canplay", handleCanPlay, { once: true });
          element.addEventListener("error", handleError, { once: true });
        });
      } catch (error) {
        if (error instanceof Error && error.message.includes("metadata")) {
          videoError = true;
        }
        return;
      }
    }

    try {
      await element.play();
    } catch {
      // A rejected `play()` is not a broken video: autoplay policy, or a pause
      // that raced the call. The element is left on its first frame with the
      // controls live, so a second press starts it.
    }
  }

  function handleVideoElementError() {
    videoError = true;
    videoLoading = false;
  }

  /// The element has a source and is filling its buffer. Distinct from
  /// `videoLoading`, which covers resolving and caching the file.
  function handleVideoWaiting() {
    videoWaitingForData = true;
  }

  function handleVideoPlaying() {
    videoWaitingForData = false;
  }

  function handleVideoLoadedData() {
    videoHasFrame = true;
  }

  async function loadFile() {
    if (fileSaved || fileLoading || !message.eventId) {
      return;
    }

    fileLoading = true;
    fileError = false;
    try {
      const { saved } = await matrixDownloadFile({
        roomId,
        eventId: message.eventId,
      });
      if (saved) {
        fileSaved = true;
      } else {
        fileError = true;
      }
    } catch {
      fileError = true;
    } finally {
      fileLoading = false;
    }
  }
</script>

<svelte:window onkeydown={handleImageViewerKeydown} onresize={fitViewerImage} onpointerdown={() => linkContextMenu = null} />

{#if message.messageType === "m.image"}
  <figure class="message-image-body space-y-2 text-[14px] leading-[1.35] font-normal">
    {#if displayImageUrl && !imageError}
      <img
        src={displayImageUrl}
        alt={message.body || "Image"}
        loading="lazy"
        role="button"
        tabindex="0"
        aria-label="View image"
        onclick={(event) => {
          event.stopPropagation();
          openImageViewer();
        }}
        onkeydown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            openImageViewer();
          }
        }}
        width={message.imageWidth ?? undefined}
        height={message.imageHeight ?? undefined}
        class="message-image-preview cursor-zoom-in preset-outlined-surface-300-700 bg-surface-100-900"
        class:message-image-preview-with-header={hasSenderHeader}
        onerror={retryMessageImage}
        oncontextmenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onImageContextMenu?.(event);
        }}
      />
    {:else}
      <span class="inline-flex items-center gap-1.5 rounded-full bg-surface-100-900/60 px-2 py-1 text-xs text-secondary" role="status">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" class="size-4 shrink-0" aria-hidden="true">
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <circle cx="8.5" cy="8.5" r="1.5" />
          <path d="m21 15-5-5L5 21" />
          <path d="m4 4 16 16" />
        </svg>
        <span>Image unavailable</span>
      </span>
    {/if}
    {#if imageCaption(message.body)}
      <figcaption class="px-[0.65rem] text-[1em] whitespace-pre-wrap break-words text-inherit">
        {imageCaption(message.body)}
      </figcaption>
    {/if}
    {#if imageViewerOpen && activeViewerMessage.imageUrl}
      <div
        class="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 p-0 backdrop-blur-sm"
        role="dialog"
        aria-modal="true"
        aria-label="Image viewer"
        tabindex="-1"
        onclick={closeImageViewer}
        onwheel={handleImageViewerWheel}
      >
        <button
          type="button"
          class="absolute right-4 top-4 rounded-full bg-black/50 px-4 py-2 text-white transition duration-150 hover:scale-110 hover:bg-white/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white active:scale-95"
          aria-label="Close image"
          onclick={(event) => {
            event.stopPropagation();
            closeImageViewer();
          }}
        >×</button>
        {#if imageMessages.length > 1}
          <button
            type="button"
            class="absolute left-4 top-1/2 z-10 grid size-12 -translate-y-1/2 place-items-center rounded-full bg-black/55 text-2xl text-white transition hover:bg-white/25 disabled:cursor-default disabled:opacity-35"
            aria-label="Previous image"
            title="Previous image"
            disabled={selectedViewerImageIndex <= 0}
            onclick={(event) => { event.stopPropagation(); navigateViewerImage(-1); }}
          >‹</button>
          <button
            type="button"
            class="absolute right-4 top-1/2 z-10 grid size-12 -translate-y-1/2 place-items-center rounded-full bg-black/55 text-2xl text-white transition hover:bg-white/25 disabled:cursor-default disabled:opacity-35"
            aria-label="Next image"
            title="Next image"
            disabled={selectedViewerImageIndex >= imageMessages.length - 1}
            onclick={(event) => { event.stopPropagation(); navigateViewerImage(1); }}
          >›</button>
        {/if}
        <div
          bind:this={viewerScrollElement}
          class="image-viewer-scrollbar max-h-[100dvh] max-w-[100vw] overflow-auto {imagePanActive ? 'cursor-grabbing' : 'cursor-grab'}"
          onclick={(event) => event.stopPropagation()}
          onpointerdown={startImagePan}
          onpointermove={moveImagePan}
          onpointerup={endImagePan}
          onpointercancel={endImagePan}
        >
          <img
            bind:this={viewerImageElement}
            src={activeViewerMessage.imageUrl ?? undefined}
            alt={activeViewerMessage.body || "Image"}
            onload={fitViewerImage}
            draggable="false"
            style={viewerImageSize ? `width:${viewerImageSize.width * imageZoom}px;height:${viewerImageSize.height * imageZoom}px` : "max-width:calc(100vw - 16px);max-height:calc(100dvh - 16px)"}
            class="block max-h-none max-w-none select-none object-contain"
          />
        </div>
        <div class="absolute bottom-4 right-4 flex items-center gap-2 rounded-full border border-white/10 bg-black/35 p-1 text-white shadow-lg" onclick={(event) => event.stopPropagation()}>
          <button type="button" class="size-9 rounded-full text-lg hover:bg-white/20" aria-label="Zoom out" title="Zoom out" onclick={() => changeImageZoom(-0.2)}>−</button>
          <button type="button" class="min-w-14 rounded-full px-2 py-1 text-xs tabular-nums hover:bg-white/20" aria-label="Reset zoom" title="Reset zoom" onclick={() => (imageZoom = 1)}>{Math.round(imageZoom * 100)}%</button>
          <button type="button" class="size-9 rounded-full text-lg hover:bg-white/20" aria-label="Zoom in" title="Zoom in" onclick={() => changeImageZoom(0.2)}>+</button>
        </div>
        <div class="absolute bottom-4 left-4 max-w-[min(56vw,44rem)] rounded-xl border border-white/10 bg-black/35 px-4 py-2 text-white shadow-lg" onclick={(event) => event.stopPropagation()}>
          <div class="flex min-w-0 items-baseline gap-2">
            <p class="min-w-0 truncate text-sm font-semibold">{activeViewerMessage.senderDisplayName || activeViewerMessage.sender}</p>
            <p class="shrink-0 text-xs font-medium text-white/75">
            {#if activeViewerMessage.timestamp}
              {new Date(activeViewerMessage.timestamp).toLocaleDateString([], { year: "numeric", month: "long", day: "numeric" })}
              ·
              {new Date(activeViewerMessage.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
            {:else}
              Date unavailable
            {/if}
            </p>
          </div>
          {#if activeViewerCaption}
            <p
              bind:this={imageCaptionElement}
              class={`mt-1 whitespace-pre-wrap break-words text-sm ${imageCaptionExpanded ? "max-h-[40vh] overflow-y-auto" : "overflow-hidden"}`}
              style={imageCaptionExpanded ? undefined : "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:2"}
            >{activeViewerCaption}</p>
            {#if imageCaptionNeedsExpansion || imageCaptionExpanded}
              <button
                type="button"
                class="mt-1 text-xs font-semibold text-white/75 hover:text-white"
                onclick={(event) => { event.stopPropagation(); imageCaptionExpanded = !imageCaptionExpanded; }}
              >{imageCaptionExpanded ? "Show less" : "Show more"}</button>
            {/if}
          {/if}
        </div>
      </div>
    {/if}
  </figure>
{:else if message.messageType === "m.video"}
  <figure class="space-y-2 text-[14px] leading-[1.35] font-normal">
    {#if videoError}
      <div class="rounded preset-outlined-surface-300-700 bg-surface-100-900 p-4 text-sm text-surface-700-300">
        Video unavailable
      </div>
    {:else}
      <div class="relative">
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          bind:this={videoElement}
          src={videoUrl ?? undefined}
          poster={videoPoster}
          controls
          playsinline
          preload="metadata"
          onclick={handleVideoPlay}
          onerror={handleVideoElementError}
          onwaiting={handleVideoWaiting}
          onplaying={handleVideoPlaying}
          oncanplay={handleVideoPlaying}
          onpause={handleVideoPlaying}
          onloadeddata={handleVideoLoadedData}
          class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 bg-surface-100-900"
        ></video>
        {#if videoLoading || videoWaitingForData}
          <div
            class="pointer-events-none absolute inset-0 flex items-center justify-center"
            aria-live="polite"
          >
            <span
              class="size-10 animate-spin rounded-full border-4 border-surface-700-300 border-t-transparent"
              role="status"
              aria-label="Loading video"
            ></span>
          </div>
        {/if}
      </div>
    {/if}
    {#if message.body}
      <figcaption class="text-[1em] whitespace-pre-wrap break-words text-surface-700-300">
        {#each splitLinks(message.body) as part, index}{#if part.url}<a href={part.url} class="cursor-pointer text-accent underline decoration-accent/50 underline-offset-2 hover:decoration-accent" onclick={(event) => { event.preventDefault(); void openUrl(part.url!); }} oncontextmenu={(event) => showLinkMenu(event, part.url!)}>{part.text}</a>{:else}{part.text}{/if}{/each}
      </figcaption>
    {/if}
  </figure>
{:else if message.messageType === "m.file"}
  <div class="rounded preset-outlined-surface-300-700 bg-surface-100-900 p-4 text-[14px] leading-[1.35] font-normal text-surface-700-300">
    {#if fileSaved}
      <span class="text-surface-700-300">File saved</span>
    {:else if fileError}
      <span class="text-surface-700-300">File unavailable</span>
    {:else}
      <button
        type="button"
        class="underline"
        onclick={loadFile}
        aria-label="Download file"
        title="Download file"
      >
        {fileLoading ? "Loading…" : "Download file"}
      </button>
    {/if}
    {#if message.body}
      <div class="mt-2 whitespace-pre-wrap break-words">{#each splitLinks(message.body) as part, index}{#if part.url}<a href={part.url} class="cursor-pointer text-accent underline decoration-accent/50 underline-offset-2 hover:decoration-accent" onclick={(event) => { event.preventDefault(); void openUrl(part.url!); }} oncontextmenu={(event) => showLinkMenu(event, part.url!)}>{part.text}</a>{:else}{part.text}{/if}{/each}</div>
    {/if}
  </div>
{:else if renderedFormattedBody}
  {#key renderedFormattedBody}
    <div use:mentionLinks use:mentionAvatars use:customEmojiFallback use:codeControls class="message-formatted-body min-w-0 max-w-full whitespace-pre-wrap break-words text-[14px] leading-[1.35] font-normal">
      {@html renderedFormattedBody}
    </div>
  {/key}
{:else}
  <p class="whitespace-pre-wrap break-words text-[14px] leading-[1.35] font-normal" class:text-danger={isUnsupportedMessage}>
    {#each splitLinks(renderedPlainBody) as part, index}{#if part.url}<a href={part.url} class="cursor-pointer text-accent underline decoration-accent/50 underline-offset-2 hover:decoration-accent" onclick={(event) => { event.preventDefault(); void openUrl(part.url!); }} oncontextmenu={(event) => showLinkMenu(event, part.url!)}>{part.text}</a>{:else}{part.text}{/if}{/each}
  </p>
{/if}

{#if linkContextMenu}
  <div class="fixed z-[120] min-w-32 border border-hairline bg-panel py-1 shadow-xl" style={`left:${linkContextMenu.x}px;top:${linkContextMenu.y}px`} role="menu" onclick={(event) => event.stopPropagation()} onpointerdown={(event) => event.stopPropagation()}>
    <button type="button" class="w-full cursor-pointer px-3 py-2 text-left text-sm text-primary hover:bg-panel-alt" role="menuitem" onclick={() => void copyContextLink()}>Copy link</button>
  </div>
{/if}

<style>
  .message-formatted-body :global(a.matrix-mention) {
    color: var(--tg-accent, #6889ff);
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--tg-accent, #6889ff) 75%, transparent);
    text-decoration-thickness: 1px;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .message-formatted-body :global(a.matrix-mention:hover) {
    color: color-mix(in srgb, var(--tg-accent, #6889ff) 82%, white);
    text-decoration-thickness: 2px;
  }
  .message-formatted-body :global(.matrix-mention-avatar) {
    display: inline-block;
    width: 1em;
    height: 1em;
    margin-right: 0.22em;
    border-radius: 50%;
    background: var(--tg-panel-alt);
    object-fit: cover;
    vertical-align: -0.16em;
  }
  .message-formatted-body :global(.matrix-mention-avatar-fallback) {
    display: inline-grid;
    width: 1em;
    height: 1em;
    margin-right: 0.22em;
    place-items: center;
    overflow: hidden;
    border-radius: 50%;
    background: var(--tg-panel-alt);
    color: var(--tg-text-secondary);
    font-size: 0.72em;
    font-weight: 600;
    line-height: 1;
    vertical-align: -0.16em;
  }
  .image-viewer-scrollbar {
    scrollbar-width: none;
    -ms-overflow-style: none;
  }
  .image-viewer-scrollbar::-webkit-scrollbar {
    display: none;
  }

  .message-image-body {
    width: calc(100% + 1.3rem);
    margin-inline: -0.65rem;
  }
  .message-image-preview {
    display: block;
    width: 100%;
    height: auto;
    max-height: 27rem;
    object-fit: contain;
    border-radius: 4px 4px 12px 12px;
  }
  .message-image-preview-with-header {
    border-radius: 4px 4px 12px 12px;
  }

  .message-formatted-body :global(.code-block) {
    min-width: 0;
    max-width: 100%;
    margin-block: 0.5rem;
    border-radius: 8px;
    background: var(--tg-panel-alt);
    color: var(--tg-text);
    overflow: hidden;
    white-space: normal;
  }
  .message-formatted-body :global(.code-copy) {
    display: block;
    margin-left: auto;
    padding: 0.4rem 0.7rem;
    font-size: 12px;
    cursor: pointer;
    color: var(--tg-accent);
  }
  .message-formatted-body :global(pre) {
    max-width: 100%;
    overflow-x: auto;
    white-space: pre;
    overflow-wrap: normal;
    word-break: normal;
    padding: 0.7rem;
    margin: 0;
    font-family: monospace;
  }
  .message-formatted-body :global(pre code) { white-space: inherit; }
  .message-formatted-body :global(blockquote) {
    border-left: 3px solid var(--tg-accent);
    padding-left: 0.6rem;
    margin-block: 0.4rem;
  }

  /* Handle both data-mx-emoticon (hyphens) and data_mx_emoticon (underscores) formats */
  .message-formatted-body :global(img[data-mx-emoticon]),
  .message-formatted-body :global(img[data_mx_emoticon]) {
    display: inline-block;
    vertical-align: text-bottom;
  }
  .message-formatted-body :global(.custom-emoji-unavailable) {
    display: inline-flex;
    align-items: center;
    gap: 0.25em;
    max-width: 8em;
    margin-inline: 0.08em;
    padding: 0.05em 0.32em;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--tg-border) 75%, transparent);
    background: color-mix(in srgb, var(--tg-panel) 80%, transparent);
    color: var(--tg-text-secondary);
    font-size: 0.72em;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: 0.08em;
  }
  .message-formatted-body :global(.custom-emoji-unavailable-icon) {
    width: 1em;
    height: 1em;
    flex: 0 0 auto;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.4;
  }
  .message-formatted-body :global(.custom-emoji-unavailable > span) {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
