<script lang="ts">
  import { iconPath } from './icons';
  export let name: string;
  export let size: number = 16;
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  {@html iconPath(name)}
</svg>
