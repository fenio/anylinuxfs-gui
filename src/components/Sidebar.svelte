<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { checkCli } from '$lib/api';

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

	.version-info {
		margin-top: auto;
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
