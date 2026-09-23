<script>
  import { fileName } from "../lib/format.js";
  import PreviewShell from "./PreviewShell.svelte";

  let {
    open = false,
    paths = [],
    title = "",
    closeLabel = "Close",
    openLabel = "Open",
    onclose,
    onopenpath,
  } = $props();

  let openingPath = $state("");

  async function handleOpen(path) {
    if (!path || openingPath) return;
    openingPath = path;
    try {
      await onopenpath?.(path);
    } finally {
      openingPath = "";
    }
  }
</script>

<PreviewShell {open} {title} {closeLabel} variant="card" {onclose}>
  <ul class="list">
    {#each paths as path (path)}
      <li>
        <button
          class="name-link"
          type="button"
          title={`${openLabel}: ${path}`}
          disabled={openingPath === path}
          onclick={() => handleOpen(path)}
        >
          {fileName(path)}
        </button>
        <div class="path" title={path}>{path}</div>
      </li>
    {/each}
  </ul>
</PreviewShell>

<style>
  .list {
    margin: 0;
    padding: 8px 0;
    overflow: auto;
    list-style: none;
    height: 100%;
  }

  .list li {
    padding: 10px 18px;
    border-bottom: 1px solid var(--border);
  }

  .list li:last-child {
    border-bottom: 0;
  }

  .name-link {
    display: inline;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    text-align: left;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    word-break: break-all;
  }

  .name-link:hover {
    filter: brightness(1.08);
  }

  .name-link:disabled {
    opacity: 0.55;
    cursor: wait;
  }

  .path {
    margin-top: 4px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
    word-break: break-all;
    line-height: 1.45;
  }
</style>
