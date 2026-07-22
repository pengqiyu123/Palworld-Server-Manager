<template>
  <div class="body">
    <slot name="sidebar" />
    <main class="main">
      <slot />
    </main>
  </div>
</template>

<script setup lang="ts">
// 纯布局壳：flex 行（侧边栏 + 主内容区），填满窗口，对齐原型 .body
</script>
