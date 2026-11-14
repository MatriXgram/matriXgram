<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
  import {
    matrixCompleteOAuth,
    matrixPasswordLogin,
    matrixSessionStatus,
    matrixStartOAuth,
  } from "../../lib/auth/api";

  import { readLastHomeserver } from "$lib/auth/homeserver";

  type SignInMethod = "oauth" | "password";

  let homeserverUrl = $state(readLastHomeserver());
  let signInMethod = $state<SignInMethod>("password");
  let homeserverExpanded = $state(false);
  const homeserverLabel = $derived.by(() => {
    try { return new URL(homeserverUrl.trim()).host || "homeserver"; }
    catch { return homeserverUrl.trim().replace(/^https?:\/\//, "").split("/")[0] || "homeserver"; }
  });
  let username = $state("");
  let password = $state("");

  let loadingSession = $state(true);
  let startingOAuth = $state(false);
  let completingOAuth = $state(false);
  let waitingForCallback = $state(false);
  let signingInWithPassword = $state(false);
  const busy = $derived(startingOAuth || completingOAuth || signingInWithPassword);

  let errorMessage = $state("");
  let infoMessage = $state("");

  let authenticated = $state(false);
  let lastHandledCallbackUrl = "";

  let browserOpened = $state(true);
  let deepLinkRegistered = $state(true);
  let authorizationUrl = $state("");
  let manualCallbackUrl = $state("");

  onMount(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      await refreshSession();

      try {
        const currentUrls = await getCurrent();
        if (!cancelled) {
          await completeOAuthFromDeepLinks(currentUrls ?? []);
        }

        unlisten = await onOpenUrl((urls) => {
          if (cancelled) {
            return;
          }

          void completeOAuthFromDeepLinks(urls);
        });
      } catch (error) {
        errorMessage = error instanceof Error ? error.message : "Failed to initialize deep-link listener";
      }
    })();

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  });

  function findOAuthCallbackUrl(urls: string[]): string | null {
    for (const urlString of urls) {
      try {
        const parsed = new URL(urlString);
        if (parsed.protocol !== "matrix:" || parsed.hostname !== "oauth-callback") {
          continue;
        }

        return urlString;
      } catch {
        continue;
      }
    }

    return null;
  }

  async function completeOAuthFromDeepLinks(urls: string[]) {
    const callbackUrl = findOAuthCallbackUrl(urls);

    if (!callbackUrl || completingOAuth || callbackUrl === lastHandledCallbackUrl) {
      return;
    }

    await completeOAuthLogin(callbackUrl);
  }

  async function refreshSession() {
    loadingSession = true;
    errorMessage = "";

    try {
      const status = await matrixSessionStatus();
      authenticated = status.authenticated;

      if (status.authenticated) {
        await goto("/chats");
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load session";
    } finally {
      loadingSession = false;
    }
  }

  function validateHomeserver(): boolean {
    try {
      const url = new URL(homeserverUrl.trim());
      if (!["http:", "https:"].includes(url.protocol) || !url.hostname || url.username || url.password) throw new Error();
      homeserverUrl = homeserverUrl.trim();
      return true;
    } catch {
      homeserverExpanded = true;
      errorMessage = "Enter a valid homeserver URL, such as https://matrix.org.";
      return false;
    }
  }

  async function startOAuthLogin(event: Event) {
    event.preventDefault();
    if (busy || !validateHomeserver()) return;
    startingOAuth = true;
    waitingForCallback = false;
    errorMessage = "";
    infoMessage = "";

    try {
      const result = await matrixStartOAuth({ homeserverUrl });
      waitingForCallback = true;
      lastHandledCallbackUrl = "";
      browserOpened = result.browserOpened;
      deepLinkRegistered = result.deepLinkRegistered;
      authorizationUrl = result.authorizationUrl;

      if (result.browserOpened && result.deepLinkRegistered) {
        infoMessage = "Browser opened. Complete sign-in to continue.";
      } else if (!result.browserOpened) {
        infoMessage =
          "Couldn't open a browser automatically. Copy the URL below and open it in your browser to sign in.";
      } else {
        infoMessage =
          "Browser opened, but the callback can't be detected automatically. Paste the callback URL below after signing in.";
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to start OAuth login";
    } finally {
      startingOAuth = false;
    }
  }

  async function startPasswordLogin(event: Event) {
    event.preventDefault();
    if (busy || !validateHomeserver()) return;
    signingInWithPassword = true;
    waitingForCallback = false;
    errorMessage = "";
    infoMessage = "";

    try {
      const response = await matrixPasswordLogin({
        homeserverUrl,
        username,
        password,
      });

      authenticated = response.authenticated;
      infoMessage = "Signed in successfully.";
      password = "";

      if (response.authenticated) {
        await goto("/chats");
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to sign in with password";
    } finally {
      signingInWithPassword = false;
    }
  }

  async function completeOAuthLogin(callbackUrl: string) {
    completingOAuth = true;
    errorMessage = "";

    try {
      const response = await matrixCompleteOAuth({ callbackUrl });
      lastHandledCallbackUrl = callbackUrl;
      authenticated = response.authenticated;
      waitingForCallback = false;
      manualCallbackUrl = "";
      infoMessage = "Signed in successfully.";

      if (response.authenticated) {
        await goto("/chats");
      }
    } catch (error) {
      waitingForCallback = false;
      errorMessage = error instanceof Error ? error.message : "Failed to complete OAuth login";
    } finally {
      completingOAuth = false;
    }
  }

  async function handleManualCallback(event: Event) {
    event.preventDefault();
    if (!manualCallbackUrl.trim()) {
      errorMessage = "Please paste the callback URL from your browser.";
      return;
    }
    await completeOAuthLogin(manualCallbackUrl.trim());
  }

</script>

<main class="flex h-full flex-col items-center overflow-y-auto bg-panel-alt p-4">
  <section class="login-card my-auto w-full max-w-[340px] shrink-0 space-y-4 rounded-xl border border-hairline bg-panel p-5 shadow-sm">
    <header class="flex items-center gap-3">
      <img src="/favicon.svg" alt="MatriXgram logo" class="size-9 shrink-0 rounded-lg" />
      <div>
        <h1 class="text-lg font-semibold leading-tight">Sign in</h1>
        <p class="mt-0.5 text-[11px] text-secondary">MatriXgram</p>
      </div>
    </header>

    {#if loadingSession}
      <p class="card p-3 text-sm bg-surface-100-900">Loading session...</p>
    {:else if authenticated}
      <p class="card p-3 text-sm bg-surface-100-900">Session active. Redirecting to chats...</p>
    {:else}
      <section class="space-y-3" aria-label="Sign in">


        {#if signInMethod === "oauth"}
          <form class="flex flex-col gap-3" onsubmit={startOAuthLogin}>
            <button
              class="btn btn-primary w-full"
              type="submit"
              disabled={startingOAuth || completingOAuth || signingInWithPassword}
            >
              {#if startingOAuth}
                Starting...
              {:else if completingOAuth}
                Completing Login...
              {:else if waitingForCallback}
                Waiting for Browser Sign-In...
              {:else}
                Continue with OAuth
              {/if}
            </button>

            {#if !browserOpened}
              <p class="text-sm text-surface-700-300">
                Couldn't open a browser automatically. Copy the URL below into your browser, then paste the callback URL back.
              </p>
            {:else if !deepLinkRegistered}
              <p class="text-sm text-surface-700-300">
                Browser opened, but the matrix:// callback can't be detected automatically. After signing in, copy the callback URL from your browser and paste it below.
              </p>
            {:else}
              <p class="text-sm text-surface-700-300">
                Sign-in completes automatically after browser authentication. No callback URL copy and paste is required.
              </p>
            {/if}

            {#if waitingForCallback && !browserOpened && authorizationUrl}
              <div class="space-y-2">
                <label class="block text-xs font-medium text-secondary" for="authUrl">Authorization URL</label>
                <input
                  class="input block w-full"
                  id="authUrl"
                  type="text"
                  readonly
                  value={authorizationUrl}
                  onclick={(e) => (e.target as HTMLInputElement).select()}
                />
                <p class="text-xs text-surface-700-300">Click to select, then copy and open in your browser.</p>
              </div>
            {/if}

            {#if waitingForCallback}
              {#if !browserOpened || !deepLinkRegistered}
                <div class="space-y-2">
                  <label class="block text-xs font-medium text-secondary" for="callbackUrl">Callback URL</label>
                  <input
                    class="input block w-full"
                    id="callbackUrl"
                    type="text"
                    bind:value={manualCallbackUrl}
                    placeholder="matrix://oauth-callback?..."
                  />
                  <button
                    class="btn preset-filled-primary-500"
                    type="button"
                    onclick={handleManualCallback}
                    disabled={completingOAuth || !manualCallbackUrl.trim()}
                  >
                    {#if completingOAuth}
                      Completing Login...
                    {:else}
                      Complete Sign-In
                    {/if}
                  </button>
                </div>
              {:else}
                <p class="text-sm text-surface-700-300">Waiting for browser callback. You can restart sign-in at any time.</p>
              {/if}
            {/if}
          </form>
          <button type="button" class="w-full text-sm font-medium text-accent hover:underline" disabled={busy}
            onclick={() => { signInMethod = "password"; waitingForCallback = false; errorMessage = ""; infoMessage = ""; }}>
            Use password instead
          </button>
        {:else}
          <form class="flex flex-col gap-3" onsubmit={startPasswordLogin}>
            <div class="grid grid-cols-[68px_minmax(0,1fr)] items-center gap-2">
              <label class="text-xs font-medium" for="username">Username</label>
              <input class="input block w-full" id="username" name="username" type="text" bind:value={username}
                autocomplete="username" autocapitalize="none" spellcheck="false" placeholder="Username / Matrix ID"
                disabled={busy} required />
            </div>
            <div class="grid grid-cols-[68px_minmax(0,1fr)] items-center gap-2">
              <label class="text-xs font-medium" for="password">Password</label>
              <input class="input block w-full" id="password" name="password" type="password" bind:value={password}
                autocomplete="current-password" placeholder="Your password" disabled={busy} required />
            </div>

            <button
              class="btn btn-primary w-full"
              type="submit"
              disabled={signingInWithPassword || startingOAuth || completingOAuth}
            >
              {#if signingInWithPassword}
                Signing In...
              {:else}
                Sign in
              {/if}
            </button>
          </form>
          <div class="flex items-center gap-3 text-xs text-muted"><span class="h-px flex-1 bg-hairline"></span>or<span class="h-px flex-1 bg-hairline"></span></div>
          <button type="button" class="btn w-full border-hairline bg-panel" disabled={busy}
            onclick={() => { signInMethod = "oauth"; errorMessage = ""; infoMessage = ""; }}>
            Sign in with OAuth
          </button>
        {/if}

        <details bind:open={homeserverExpanded} class="homeserver-selector">
          <summary class="flex cursor-pointer list-none items-center justify-between gap-3 rounded-md px-1 py-1 text-xs hover:bg-hover">
            <span class="min-w-0 truncate text-secondary">Using <strong class="font-medium text-primary">{homeserverLabel}</strong></span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="homeserver-chevron size-4 shrink-0 text-secondary" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>
          </summary>
          <div class="space-y-2 border-t border-hairline p-3">
            <label class="block text-xs font-medium text-secondary" for="homeserver">Homeserver URL</label>
            <input class="input block w-full" id="homeserver" type="url" bind:value={homeserverUrl}
              placeholder="https://matrix.org" autocomplete="url" spellcheck="false" disabled={busy} required />
          </div>
        </details>
      </section>
    {/if}

    {#if errorMessage}
      <p role="alert" class="rounded-xl bg-danger-soft p-3 text-sm text-danger">{errorMessage}</p>
    {/if}

    {#if infoMessage}
      <p role="status" class="rounded-xl bg-accent-soft p-3 text-sm text-accent">{infoMessage}</p>
    {/if}
  </section>
</main>

<style>
  .login-card :global(.input) {
    min-height: 32px;
    padding: 6px 9px;
    border-radius: 7px;
    font-size: 13px;
    line-height: 1.35;
  }
  .login-card :global(.btn) {
    min-height: 32px;
    padding: 6px 10px;
    border-radius: 7px;
    font-size: 13px;
  }

  .homeserver-selector summary::-webkit-details-marker { display: none; }
  .homeserver-chevron { transition: transform 150ms ease; }
  .homeserver-selector[open] .homeserver-chevron { transform: rotate(180deg); }
  @media (prefers-reduced-motion: reduce) { .homeserver-chevron { transition: none; } }
</style>
