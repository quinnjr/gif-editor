<script lang="ts">
  import '../app.css';
  import { project } from '$lib/stores/project.svelte';

  let { children } = $props();

  function handleKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key === 'z' && !e.shiftKey) {
      e.preventDefault();
      project.undo().catch(console.error);
    } else if (mod && (e.key === 'y' || (e.key === 'z' && e.shiftKey))) {
      e.preventDefault();
      project.redo().catch(console.error);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />
{@render children()}
