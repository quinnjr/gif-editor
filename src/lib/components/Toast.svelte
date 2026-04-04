<script lang="ts">
  let { message, type = 'error' }: { message: string; type?: 'error' | 'success' } = $props();
  let visible = $state(true);

  $effect(() => {
    const timer = setTimeout(() => (visible = false), 4000);
    return () => clearTimeout(timer);
  });
</script>

{#if visible}
  <div
    class="fixed bottom-4 right-4 z-50 rounded-lg px-4 py-3 text-sm font-medium shadow-lg
           {type === 'error' ? 'bg-red-600 text-white' : 'bg-green-600 text-white'}"
  >
    {message}
  </div>
{/if}
