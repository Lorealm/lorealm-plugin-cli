import { defineConfig } from 'vite';
import { viteSingleFile } from 'vite-plugin-singlefile';

// A frame can't load anything from outside its HTML file, so everything is
// inlined into it. The client gets what's in `dist`.
export default defineConfig({
  plugins: [viteSingleFile()],
  build: {
    rollupOptions: {
      input: '{{slot}}.html',
    },
  },
});
