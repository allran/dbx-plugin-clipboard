<script>
  import { onDestroy, onMount } from "svelte";
  import ClearModal from "./components/ClearModal.svelte";
  import FileViewer from "./components/FileViewer.svelte";
  import HistoryItem from "./components/HistoryItem.svelte";
  import ImageLightbox from "./components/ImageLightbox.svelte";
  import TextViewer from "./components/TextViewer.svelte";
  import Toast from "./components/Toast.svelte";
  import { itemMatches } from "./lib/format.js";
  import { resolveMessages } from "./lib/i18n.js";

  let text = $state(resolveMessages("en"));
  let items = $state([]);
  let query = $state("");
  let bootError = $state("");
  let refreshing = $state(false);
  let clearOpen = $state(false);
  let clearing = $state(false);
  let toastMessage = $state("");
  let toastVisible = $state(false);
  /** @type {null | { kind: "image"; src: string } | { kind: "text"; content: string } | { kind: "files"; paths: string[] }} */
  let preview = $state(null);
  let kindFilter = $state("all");

  let toastTimer = 0;
  let unsubscribeEvent = null;

  const tabs = $derived([
    { id: "all", label: text.tabAll },
    { id: "text", label: text.kindText },
    { id: "image", label: text.kindImage },
    { id: "files", label: text.kindFiles },
  ]);

  const needle = $derived(query.trim().toLowerCase());

  const filtered = $derived(
    items.filter((item) => {
      const kind = item.kind || "text";
      if (kindFilter !== "all" && kind !== kindFilter) return false;
      return itemMatches(item, needle);
    }),
  );

  const hasFilter = $derived(Boolean(needle) || kindFilter !== "all");

  function applyLocale() {
    text = resolveMessages(window.dbxPlugin?.locale || navigator.language || "en");
  }

  function showToast(message) {
    toastMessage = message;
    toastVisible = true;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastVisible = false;
    }, 1800);
  }

  function closePreview() {
    preview = null;
  }

  async function loadList() {
    const result = await window.dbxPlugin.invoke("clipboard/list", {});
    items = Array.isArray(result?.items) ? result.items : [];
  }

  async function copyItem(item) {
    try {
      await window.dbxPlugin.invoke("clipboard/copy", { id: item.id });
      showToast(text.copied);
      await loadList();
    } catch (error) {
      showToast(`${text.error}: ${error.message}`);
    }
  }

  async function deleteItem(item) {
    try {
      await window.dbxPlugin.invoke("clipboard/delete", { id: item.id });
      showToast(text.deleted);
      await loadList();
    } catch (error) {
      showToast(`${text.error}: ${error.message}`);
    }
  }

  async function refresh() {
    refreshing = true;
    try {
      await window.dbxPlugin.invoke("clipboard/capture", {});
      await loadList();
      showToast(text.refreshed);
    } catch (error) {
      showToast(`${text.error}: ${error.message}`);
    } finally {
      refreshing = false;
    }
  }

  async function performClear() {
    clearing = true;
    try {
      await window.dbxPlugin.invoke("clipboard/clear", {});
      clearOpen = false;
      showToast(text.cleared);
      await loadList();
    } catch (error) {
      showToast(`${text.error}: ${error.message}`);
    } finally {
      clearing = false;
    }
  }

  async function previewImage(item) {
    try {
      const result = await window.dbxPlugin.invoke("clipboard/media", { id: item.id });
      const src = result?.dataUrl || "";
      if (!src) throw new Error("Image unavailable");
      preview = { kind: "image", src };
    } catch (error) {
      showToast(`${text.error}: ${error.message}`);
    }
  }

  function openText(item) {
    preview = { kind: "text", content: item?.text || "" };
  }

  function openFiles(item) {
    preview = {
      kind: "files",
      paths: Array.isArray(item?.paths) ? item.paths : [],
    };
  }

  async function openLocalPath(path) {
    try {
      await window.dbxPlugin.invoke("clipboard/open-path", { path });
      showToast(text.opened);
    } catch (error) {
      showToast(`${text.error}: ${error.message}`);
    }
  }

  onMount(() => {
    window.dbxPlugin.ready.then(async () => {
      applyLocale();
      window.addEventListener("dbx-plugin-env", applyLocale);

      try {
        unsubscribeEvent =
          window.dbxPlugin.onEvent?.((event) => {
            const method = event?.method || event?.type || event?.name || event?.event;
            if (method === "clipboard/item") {
              loadList().catch(() => {});
            }
          }) ?? null;
      } catch {
        // events optional in older hosts
      }

      try {
        await window.dbxPlugin.invoke("clipboard/capture", {});
        await loadList();
      } catch (error) {
        bootError = `${text.error}: ${error.message}`;
      }
    });
  });

  onDestroy(() => {
    clearTimeout(toastTimer);
    window.removeEventListener("dbx-plugin-env", applyLocale);
    unsubscribeEvent?.();
  });
</script>

<svelte:head>
  <title>Clipboard History</title>
</svelte:head>

<div class="app">
  <div class="toolbar">
    <input
      class="search"
      type="search"
      autocomplete="off"
      spellcheck="false"
      placeholder={text.search}
      bind:value={query}
    />
    <button
      class="icon-btn"
      type="button"
      title={text.refresh}
      aria-label={text.refresh}
      disabled={refreshing}
      onclick={refresh}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
        <path
          fill="currentColor"
          d="M17.65 6.35A7.95 7.95 0 0 0 12 4a8 8 0 1 0 7.75 10h-2.1A6 6 0 1 1 12 6c1.57 0 2.98.61 4.04 1.61L13 10h7V3z"
        />
      </svg>
    </button>
    <button
      class="icon-btn danger"
      type="button"
      title={text.clear}
      aria-label={text.clear}
      onclick={() => (clearOpen = true)}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
        <path
          fill="currentColor"
          d="M9 3h6a1 1 0 0 1 1 1v1h4v2h-1v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7H4V5h4V4a1 1 0 0 1 1-1Zm1 2v0h4V5h-4Zm-2 4v10h8V9H8Z"
        />
      </svg>
    </button>
  </div>

  <div class="tabs" role="tablist" aria-label="Filter">
    {#each tabs as tab (tab.id)}
      <button
        class="tab"
        class:active={kindFilter === tab.id}
        type="button"
        role="tab"
        aria-selected={kindFilter === tab.id}
        onclick={() => (kindFilter = tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="meta">
    <span title={text.hint}>{text.count(filtered.length)} · {text.hint}</span>
  </div>

  <div class="list" role="list">
    {#if bootError}
      <div class="error">{bootError}</div>
    {:else if filtered.length === 0}
      <div class="empty">{hasFilter ? text.emptyFilter : text.empty}</div>
    {:else}
      {#each filtered as item (item.id)}
        <HistoryItem
          {item}
          {text}
          oncopy={copyItem}
          ondelete={deleteItem}
          onpreview={previewImage}
          onviewtext={openText}
          onviewfiles={openFiles}
        />
      {/each}
    {/if}
  </div>
</div>

<Toast message={toastMessage} visible={toastVisible} />

<ClearModal
  open={clearOpen}
  title={text.clearTitle}
  body={text.clearBody}
  cancelLabel={text.cancel}
  confirmLabel={text.clearConfirm}
  confirming={clearing}
  oncancel={() => (clearOpen = false)}
  onconfirm={performClear}
/>

<ImageLightbox
  open={preview?.kind === "image"}
  src={preview?.kind === "image" ? preview.src : ""}
  alt={text.kindImage}
  closeLabel={text.close}
  onclose={closePreview}
/>

<TextViewer
  open={preview?.kind === "text"}
  content={preview?.kind === "text" ? preview.content : ""}
  title={text.textDetail}
  closeLabel={text.close}
  onclose={closePreview}
/>

<FileViewer
  open={preview?.kind === "files"}
  paths={preview?.kind === "files" ? preview.paths : []}
  title={text.filesDetail}
  closeLabel={text.close}
  openLabel={text.openPath}
  onclose={closePreview}
  onopenpath={openLocalPath}
/>

<style>
  :global(:root) {
    color-scheme: light dark;
    --bg: var(--color-background, Canvas);
    --fg: var(--color-foreground, CanvasText);
    --muted: color-mix(in srgb, var(--fg) 55%, transparent);
    --border: color-mix(in srgb, var(--fg) 14%, transparent);
    --hover: color-mix(in srgb, var(--fg) 6%, transparent);
    --active: color-mix(in srgb, var(--fg) 10%, transparent);
    --accent: var(--color-primary, #2563eb);
    --danger: #dc2626;
    --radius: var(--radius-md, 10px);
    --font: var(--font-sans, ui-sans-serif, system-ui, -apple-system, sans-serif);
    --mono: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(html),
  :global(body) {
    margin: 0;
    height: 100%;
    background: var(--bg);
    color: var(--fg);
    font: 13px/1.45 var(--font);
  }

  :global(#app) {
    height: 100%;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .toolbar {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .search {
    flex: 1;
    min-width: 0;
    height: 34px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: inherit;
    font: inherit;
    outline: none;
  }

  .search:focus {
    border-color: color-mix(in srgb, var(--accent) 70%, var(--border));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .icon-btn {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: inherit;
    cursor: pointer;
    flex-shrink: 0;
  }

  .icon-btn:hover {
    background: var(--hover);
  }

  .icon-btn:disabled {
    opacity: 0.5;
    cursor: wait;
  }

  .icon-btn.danger {
    color: var(--danger);
  }

  .icon-btn.danger:hover {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 8px 14px 0;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tab {
    height: 30px;
    padding: 0 12px;
    border: 0;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--fg);
  }

  .tab.active {
    color: var(--fg);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-size: 12px;
    flex-shrink: 0;
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .empty,
  .error {
    display: grid;
    place-items: center;
    height: 100%;
    padding: 32px;
    text-align: center;
    color: var(--muted);
    white-space: pre-line;
  }

  .error {
    color: var(--danger);
  }
</style>
