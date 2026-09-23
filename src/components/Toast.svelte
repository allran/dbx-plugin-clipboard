<script>
  let { message = "", visible = false } = $props();
</script>

<div class="toast" class:show={visible} role="status" aria-live="polite">{message}</div>

<style>
  .toast {
    position: fixed;
    left: 50%;
    bottom: 24px;
    z-index: 40;
    transform: translateX(-50%) translateY(8px);
    max-width: min(420px, calc(100% - 32px));
    padding: 10px 16px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: color-mix(in srgb, var(--bg) 88%, var(--fg));
    color: var(--fg);
    box-shadow: 0 10px 28px color-mix(in srgb, #000 18%, transparent);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 160ms ease,
      transform 160ms ease;
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .toast.show {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
</style>
