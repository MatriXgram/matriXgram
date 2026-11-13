<script lang="ts">
  import "../app.css";
  import { initialiseTheme, watchSystemTheme } from "$lib/theme";

  let { children } = $props();

  // The palette is applied before the first paint by the inline script in
  // app.html; this restores the user's overrides and the loaded theme.
  initialiseTheme();
  watchSystemTheme();
</script>

{@render children()}
