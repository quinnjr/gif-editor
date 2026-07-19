<script lang="ts">
  let { message, type = 'error', nonce = 0 }: {
    message: string;
    type?: 'error' | 'success';
    nonce?: number;
  } = $props();
  let visible = $state(false);

  // Re-show the toast for every new message. Depends on both `message` and
  // `nonce` so a repeated identical message (same string, incremented nonce)
  // also re-shows and restarts the 4s hide timer.
  $effect(() => {
    // Read (not just receive) `nonce` so it is tracked as a dependency —
    // a repeated identical message only changes the nonce.
    void nonce;
    if (message === '') {
      visible = false;
      return;
    }
    visible = true;
    const timer = setTimeout(() => (visible = false), 4000);
    return () => clearTimeout(timer);
  });
</script>

{#if visible}
  <div
    class="fixed bottom-4 right-4 z-50 rounded-lg border px-4 py-3 text-sm font-medium shadow-lg
           {type === 'error'
             ? 'border-red-500/40 bg-ember text-ink'
             : 'border-lamp/40 bg-raise text-ink'}"
  >
    {message}
  </div>
{/if}
