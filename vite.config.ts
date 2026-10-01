import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit({
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html',
			precompress: false,
			strict: true
		}),
		paths: {
			base: '',
			relative: true
		},
		appDir: '_app'
	})],
	clearScreen: false,
	server: {
		port: 2137,
		strictPort: true
	},
	build: {
		target: 'esnext'
	}
});
