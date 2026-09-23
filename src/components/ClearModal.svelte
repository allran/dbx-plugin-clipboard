<script>
  let {
    open = false,
    title = "",
    body = "",
    cancelLabel = "Cancel",
    confirmLabel = "Clear all",
    confirming = false,
    oncancel,
    onconfirm,
  } = $props();

  function onKeydown(event) {
    if (event.key === "Escape" && open) oncancel?.();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal">
    <button class="modal-backdrop" type="button" aria-label={cancelLabel} onclick={() => oncancel?.()}></button>
    <div class="modal-panel" role="dialog" aria-modal="true" aria-labelledby="clear-title">
      <h2 id="clear-title" class="modal-title">{title}</h2>
      <p class="modal-body">{body}</p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => oncancel?.()}>{cancelLabel}</button>
        <button class="btn btn-primary" type="button" disabled={confirming} onclick={() => onconfirm?.()}>
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 20px;
  }

  .modal-backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: color-mix(in srgb, #000 45%, transparent);
    cursor: pointer;
  }

  .modal-panel {
    position: relative;
    width: min(380px, 100%);
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--bg);
    box-shadow: 0 18px 50px color-mix(in srgb, #000 28%, transparent);
  }

  .modal-title {
    margin: 0 0 8px;
    font-size: 16px;
    font-weight: 650;
  }

  .modal-body {
    margin: 0 0 18px;
    color: var(--muted);
    line-height: 1.55;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn {
    height: 34px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
    white-space: nowrap;
  }

  .btn:hover {
    background: var(--hover);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: wait;
  }

  .btn-primary {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  .btn-primary:hover {
    filter: brightness(0.95);
    background: var(--danger);
  }
</style>
