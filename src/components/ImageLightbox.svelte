<script>
  import PreviewShell from "./PreviewShell.svelte";

  let { open = false, src = "", alt = "", closeLabel = "Close", onclose } = $props();
</script>

<PreviewShell open={open && !!src} {closeLabel} variant="media" {onclose}>
  <img {src} {alt} />
</PreviewShell>

<style>
  img {
    display: block;
    width: auto;
    height: auto;
    max-width: 100%;
    max-height: min(92vh, 100%);
    border-radius: 12px;
    background: #111;
    object-fit: contain;
  }
</style>
