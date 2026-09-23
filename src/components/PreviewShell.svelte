<script>
  let {
    open = false,
    title = "",
    closeLabel = "Close",
    variant = "card",
    onclose,
    children,
  } = $props();

  function onKeydown(event) {
    if (event.key === "Escape" && open) onclose?.();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="overlay">
    <button class="backdrop" type="button" aria-label={closeLabel} onclick={() => onclose?.()}></button>
    <div
      class="panel"
      class:card={variant === "card"}
      class:media={variant === "media"}
      role="dialog"
      aria-modal="true"
      aria-label={title || undefined}
    >
      <button class="close" type="button" aria-label={closeLabel} onclick={() => onclose?.()}>
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
          <path
            fill="currentColor"
            d="M18.3 5.7a1 1 0 0 0-1.4-1.4L12 9.17 7.1 4.3A1 1 0 0 0 5.7 5.7L10.59 10.6 5.7 15.49a1 1 0 1 0 1.4 1.42L12 12l4.9 4.9a1 1 0 0 0 1.4-1.42L13.41 10.6z"
          />
        </svg>
      </button>

      {#if title && variant === "card"}
        <div class="header">{title}</div>
      {/if}

      <div class="body">
        {@render children?.()}
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    padding: 16px;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: color-mix(in srgb, #000 72%, transparent);
    cursor: pointer;
  }

  .panel {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 60px color-mix(in srgb, #000 35%, transparent);
  }

  .panel.card {
    width: min(860px, 96vw);
    max-height: min(88vh, 100%);
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--bg);
  }

  .panel.media {
    width: min(96vw, 1600px);
    max-height: min(92vh, 100%);
    border-radius: 12px;
    background: transparent;
  }

  .close {
    position: absolute;
    top: 10px;
    right: 10px;
    z-index: 2;
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border: 1px solid color-mix(in srgb, var(--fg) 14%, transparent);
    border-radius: 999px;
    background: var(--bg);
    color: var(--fg);
    box-shadow: 0 4px 14px color-mix(in srgb, #000 22%, transparent);
    cursor: pointer;
  }

  .panel.media .close {
    border-color: rgba(255, 255, 255, 0.55);
    background: #ffffff;
    color: #111111;
  }

  .close:hover {
    filter: brightness(0.96);
  }

  .header {
    flex-shrink: 0;
    padding: 14px 48px 14px 18px;
    border-bottom: 1px solid var(--border);
    font-size: 13px;
    font-weight: 600;
  }

  .body {
    min-height: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .panel.media .body {
    align-items: center;
    justify-content: center;
  }
</style>
