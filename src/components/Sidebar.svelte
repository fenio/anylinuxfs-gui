<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { checkCli } from '$lib/api';
	import { open } from '@tauri-apps/plugin-shell';
	import { exit } from '@tauri-apps/plugin-process';

	const navItems = [
		{ path: '/', label: 'Disks', icon: 'disk' },
		{ path: '/shell', label: 'Shell', icon: 'shell' },
		{ path: '/images', label: 'Images', icon: 'image' },
		{ path: '/packages', label: 'Packages', icon: 'package' },
		{ path: '/actions', label: 'Actions', icon: 'action' },
		{ path: '/logs', label: 'Logs', icon: 'log' },
		{ path: '/settings', label: 'Settings', icon: 'settings' }
	];

	let guiVersion = $state('');
	let cliVersion = $state<string | null>(null);

	function isActive(path: string, currentPath: string): boolean {
		if (path === '/') {
			return currentPath === '/';
		}
		return currentPath.startsWith(path);
	}

	onMount(async () => {
		const status = await checkCli();
		guiVersion = status.gui_version;
		cliVersion = status.cli_version;
	});
</script>

<nav class="sidebar">
	<div class="sidebar-header">
		<img src="./logo.png" alt="" class="app-logo" />
		<h1 class="app-title">anylinuxfs</h1>
		<span class="app-subtitle">Linux filesystems on macOS</span>
	</div>
	<ul class="nav-list">
		{#each navItems as item}
			<li>
				<a
					href={item.path}
					class="nav-item"
					class:active={isActive(item.path, $page.url.pathname)}
				>
					<span class="nav-icon" data-icon={item.icon}></span>
					<span class="nav-label">{item.label}</span>
				</a>
			</li>
		{/each}
	</ul>

	<button class="quit-btn" onclick={() => exit(0)}>
		<span class="quit-icon"></span>
		Quit
	</button>

	<button
		class="github-link"
		onclick={() => open('https://github.com/fenio/anylinuxfs-gui')}
	>
		<svg class="github-icon" viewBox="0 0 16 16" fill="currentColor">
			<path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z"/>
		</svg>
		View on GitHub &#11088;
	</button>

	<div class="version-info">
		<span class="version-label">Versions:</span>
		{#if guiVersion}
			<span>GUI: {guiVersion}</span>
		{/if}
		{#if cliVersion}
			<span>CLI: {cliVersion}</span>
		{/if}
	</div>
</nav>

<style>
	.sidebar {
		width: 200px;
		background: var(--sidebar-bg);
		border-right: 1px solid var(--border-color);
		display: flex;
		flex-direction: column;
		-webkit-app-region: drag;
	}

	.sidebar-header {
		padding: 20px 16px;
		border-bottom: 1px solid var(--border-color);
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		text-align: center;
	}

	.app-logo {
		width: 128px;
		height: 128px;
	}

	.app-title {
		font-size: 15px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.app-subtitle {
		font-size: 10px;
		color: var(--text-secondary);
		line-height: 1.3;
	}

	.nav-list {
		list-style: none;
		padding: 8px;
		margin: 0;
		-webkit-app-region: no-drag;
	}

	.nav-item {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 12px;
		border-radius: 6px;
		color: var(--text-secondary);
		text-decoration: none;
		font-size: 13px;
		transition: background-color 0.15s, color 0.15s;
	}

	.nav-item:hover {
		background: var(--hover-bg);
		color: var(--text-primary);
	}

	.nav-item.active {
		background: var(--accent-color);
		color: white;
	}

	.nav-icon {
		width: 20px;
		height: 20px;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 18px;
	}

	.nav-icon[data-icon='disk']::before {
		content: '\1F4BE';
	}

	.nav-icon[data-icon='log']::before {
		content: '\1F4C4';
	}

	.nav-icon[data-icon='settings']::before {
		content: '\2699';
		font-size: 20px;
	}

	.nav-icon[data-icon='shell']::before {
		content: '>';
		font-family: monospace;
		font-weight: bold;
		font-size: 16px;
	}

	.nav-icon[data-icon='image']::before {
		content: '\1F4BF';
	}

	.nav-icon[data-icon='package']::before {
		content: '\1F4E6';
	}

	.nav-icon[data-icon='action']::before {
		content: '\26A1';
	}

	.quit-btn {
		display: flex;
		align-items: center;
		gap: 10px;
		margin: 40px 8px 0;
		padding: 8px 12px;
		border-radius: 6px;
		border: none;
		background: none;
		color: var(--text-secondary);
		font-size: 13px;
		cursor: pointer;
		-webkit-app-region: no-drag;
		transition: background-color 0.15s, color 0.15s;
	}

	.quit-btn:hover {
		background: var(--hover-bg);
		color: var(--error-color);
	}

	.quit-icon::before {
		content: '\23FB';
		font-size: 18px;
		display: flex;
		width: 20px;
		height: 20px;
		align-items: center;
		justify-content: center;
	}

	.github-link {
		margin-top: auto;
		padding: 8px 16px;
		background: none;
		border: none;
		color: var(--text-secondary);
		font-size: 11px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 5px;
		-webkit-app-region: no-drag;
	}

	.github-icon {
		width: 14px;
		height: 14px;
	}

	.github-link:hover {
		color: var(--text-primary);
	}

	.version-info {
		padding: 10px 16px;
		border-top: 1px solid var(--border-color);
		display: flex;
		flex-direction: column;
		gap: 2px;
		font-size: 10px;
		color: var(--text-tertiary);
		-webkit-app-region: no-drag;
	}

	.version-label {
		color: var(--text-secondary);
		margin-bottom: 2px;
	}
</style>
