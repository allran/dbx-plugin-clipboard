<script>
  import PreviewShell from "./PreviewShell.svelte";

  let {
    open = false,
    content = "",
    title = "",
    closeLabel = "Close",
    onclose,
  } = $props();
</script>

<PreviewShell {open} {title} {closeLabel} variant="card" {onclose}>
  <pre class="content">{content}</pre>
</PreviewShell>

<style>
  .content {
    margin: 0;
    padding: 16px 18px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--mono);
    font-size: 13px;
    line-height: 1.55;
    height: 100%;
  }
</style>
