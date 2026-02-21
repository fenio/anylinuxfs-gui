import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	clearScreen: false,
	server: {
		port: 2137,
		strictPort: true
	},
	build: {
		target: 'esnext'
	},
	// Use relative paths for Tauri
	base: './'
});
