<script>
  import { onDestroy } from "svelte";
  import { fileName, formatTime, previewText } from "../lib/format.js";

  let { item, text, oncopy, ondelete, onpreview, onviewtext, onviewfiles } = $props();

  let copied = $state(false);
  let viewTimer = 0;
  let copyTimer = 0;

  function kindLabel(kind) {
    if (kind === "image") return text.kindImage;
    if (kind === "files") return text.kindFiles;
    return text.kindText;
  }

  function metaLabel(current) {
    if (current.kind === "image") {
      return text.imageSize(current.width || "?", current.height || "?");
    }
    if (current.kind === "files") {
      const n = Array.isArray(current.paths) ? current.paths.length : current.charCount || 0;
      return text.filesCount(n);
    }
    return text.chars(current.charCount ?? (current.text || "").length);
  }

  function scheduleView(action) {
    return (event) => {
      event.stopPropagation();
      clearTimeout(viewTimer);
      // Delay single-click view so a double-click can cancel it and only copy.
      viewTimer = setTimeout(action, 220);
    };
  }

  async function handleCopy() {
    clearTimeout(viewTimer);
    await oncopy?.(item);
    copied = true;
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copied = false;
    }, 500);
  }

  onDestroy(() => {
    clearTimeout(viewTimer);
    clearTimeout(copyTimer);
  });
</script>

<div
  class="row"
  class:copied
  role="listitem"
  title={text.hint}
  ondblclick={handleCopy}
>
  <div class="content">
    {#if item.kind === "image" && item.previewDataUrl}
      <button
        class="thumb-btn"
        type="button"
        title={text.viewImage}
        onclick={scheduleView(() => onpreview?.(item))}
      >
        <img class="thumb" src={item.previewDataUrl} alt={text.kindImage} />
      </button>
    {:else if item.kind === "files" && Array.isArray(item.paths) && item.paths.length}
      <button
        class="text-btn"
        type="button"
        title={text.viewFiles}
        onclick={scheduleView(() => onviewfiles?.(item))}
      >
        <ul class="file-list">
          {#each item.paths.slice(0, 8) as path (path)}
            <li title={path}>{fileName(path)}</li>
          {/each}
          {#if item.paths.length > 8}
            <li>… +{item.paths.length - 8}</li>
          {/if}
        </ul>
      </button>
    {:else}
      <button
        class="text-btn"
        type="button"
        title={text.viewText}
        onclick={scheduleView(() => onviewtext?.(item))}
      >
        <pre class="preview">{previewText(item.text)}</pre>
      </button>
    {/if}
  </div>

  <div class="footer">
    <div class="meta">
      <span class="kind-badge">{kindLabel(item.kind)}</span>
      <span>{formatTime(item.createdAt)}</span>
      <span>{metaLabel(item)}</span>
    </div>
    <button
      class="delete"
      type="button"
      title={text.delete}
      aria-label={text.delete}
      onclick={(event) => {
        event.stopPropagation();
        ondelete?.(item);
      }}
    >
      <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
        <path
          fill="currentColor"
          d="M9 3h6a1 1 0 0 1 1 1v1h4v2h-1v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7H4V5h4V4a1 1 0 0 1 1-1Zm1 2v0h4V5h-4Zm-2 4v10h8V9H8Z"
        />
      </svg>
    </button>
  </div>
</div>

<style>
  .row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
    cursor: default;
    user-select: none;
  }

  .row:hover {
    background: var(--hover);
  }

  .row:active {
    background: var(--active);
  }

  .row.copied {
    outline: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .content {
    min-width: 0;
  }

  .text-btn {
    display: block;
    width: 100%;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .preview {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.5;
    max-height: 4.5em;
    overflow: hidden;
  }

  .thumb-btn {
    display: inline-flex;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: zoom-in;
  }

  .thumb {
    display: block;
    max-width: min(220px, 100%);
    max-height: 120px;
    width: auto;
    height: auto;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--fg) 4%, transparent);
    object-fit: contain;
  }

  .file-list {
    margin: 0;
    padding-left: 18px;
    font-family: var(--mono);
    font-size: 12px;
    line-height: 1.5;
    color: var(--fg);
    pointer-events: none;
  }

  .file-list li {
    word-break: break-all;
  }

  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: var(--muted);
    font-size: 11px;
  }

  .kind-badge {
    display: inline-flex;
    align-items: center;
    padding: 1px 7px;
    border-radius: 999px;
    border: 1px solid var(--border);
    color: var(--muted);
    font-size: 11px;
  }

  .delete {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    border-radius: 8px;
  }

  .delete:hover {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
</style>
