<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Terminal } from '@xterm/xterm';
	import { FitAddon } from '@xterm/addon-fit';
	import { listen } from '@tauri-apps/api/event';
	import { startShell, writeShell, resizeShell, stopShell, getMountStatus } from '$lib/api';
	import '@xterm/xterm/css/xterm.css';

	let terminalEl: HTMLDivElement;
	let terminal: Terminal | null = null;
	let fitAddon: FitAddon | null = null;
	let running = $state(false);
	let error = $state<string | null>(null);
	let isMounted = $state(false);
	let selectedImage = $state('alpine');
	let unlistenOutput: (() => void) | null = null;
	let unlistenExit: (() => void) | null = null;

	async function checkMountStatus() {
		try {
			const status = await getMountStatus();
			isMounted = status.mounted;
		} catch {
			isMounted = false;
		}
	}

	onMount(async () => {
		terminal = new Terminal({
			cursorBlink: true,
			fontSize: 14,
			fontFamily: 'Menlo, Monaco, "Courier New", monospace',
			theme: {
				background: '#1e1e1e',
				foreground: '#d4d4d4',
				cursor: '#d4d4d4',
				cursorAccent: '#1e1e1e',
				selectionBackground: '#264f78',
				black: '#000000',
				red: '#cd3131',
				green: '#0dbc79',
				yellow: '#e5e510',
				blue: '#2472c8',
				magenta: '#bc3fbc',
				cyan: '#11a8cd',
				white: '#e5e5e5',
				brightBlack: '#666666',
				brightRed: '#f14c4c',
				brightGreen: '#23d18b',
				brightYellow: '#f5f543',
				brightBlue: '#3b8eea',
				brightMagenta: '#d670d6',
				brightCyan: '#29b8db',
				brightWhite: '#ffffff'
			}
		});

		fitAddon = new FitAddon();
		terminal.loadAddon(fitAddon);
		terminal.open(terminalEl);
		fitAddon.fit();

		// Handle user input
		terminal.onData((data) => {
			if (running) {
				writeShell(data).catch(console.error);
			}
		});

		// Handle resize
		const resizeObserver = new ResizeObserver(() => {
			if (fitAddon && terminal) {
				fitAddon.fit();
				if (running) {
					resizeShell(terminal.rows, terminal.cols).catch(console.error);
				}
			}
		});
		resizeObserver.observe(terminalEl);

		// Listen for shell output
		unlistenOutput = await listen<string>('shell-output', (event) => {
			terminal?.write(event.payload);
		});

		// Listen for shell exit
		unlistenExit = await listen('shell-exit', () => {
			running = false;
			terminal?.writeln('\r\n\x1b[33m[Shell exited. Click "Start Shell" to reconnect.]\x1b[0m');
		});

		// Check mount status
		await checkMountStatus();

		return () => {
			resizeObserver.disconnect();
		};
	});

	onDestroy(() => {
		if (running) {
			stopShell().catch(console.error);
		}
		unlistenOutput?.();
		unlistenExit?.();
		terminal?.dispose();
	});

	async function handleStart() {
		if (running) return;

		// Recheck mount status before starting
		await checkMountStatus();
		if (isMounted) {
			terminal?.writeln('\x1b[33mCannot start shell while a filesystem is mounted.\x1b[0m\r\n');
			terminal?.writeln('\x1b[33mPlease unmount first from the Disks page.\x1b[0m\r\n');
			return;
		}

		error = null;
		terminal?.clear();
		terminal?.writeln(`Starting ${selectedImage} shell...\r\n`);

		try {
			await startShell(selectedImage);
			running = true;
			// Send initial resize
			if (terminal) {
				await resizeShell(terminal.rows, terminal.cols);
			}
		} catch (e) {
			error = String(e);
			terminal?.writeln(`\x1b[31mError: ${error}\x1b[0m\r\n`);
		}
	}

	async function handleStop() {
		if (!running) return;

		try {
			await stopShell();
			running = false;
			terminal?.writeln('\r\n\x1b[33m[Shell stopped.]\x1b[0m');
		} catch (e) {
			console.error('Stop error:', e);
		}
	}
</script>

<svelte:head>
	<title>Shell - anylinuxfs</title>
</svelte:head>

<div class="shell-page">
	<div class="header">
		<h2>VM Shell</h2>
		<div class="actions">
			{#if isMounted}
				<span class="status-badge warning">Filesystem mounted</span>
			{:else if !running}
				<select class="image-select" bind:value={selectedImage}>
					<option value="alpine">Alpine Linux</option>
					<option value="freebsd">FreeBSD</option>
				</select>
				<button class="btn-primary" onclick={handleStart}>Start Shell</button>
			{:else}
				<span class="status-badge">Connected</span>
				<button class="btn-secondary" onclick={handleStop}>Stop</button>
			{/if}
		</div>
	</div>

	{#if isMounted}
		<div class="warning-banner">
			<span>Shell is unavailable while a filesystem is mounted. Please unmount first from the Disks page.</span>
		</div>
	{/if}

	{#if error}
		<div class="error-banner">
			<span>{error}</span>
			<button onclick={() => (error = null)}>Dismiss</button>
		</div>
	{/if}

	<div class="terminal-container">
		<div class="terminal" bind:this={terminalEl}></div>
	</div>
</div>

<style>
	.shell-page {
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}

	.header h2 {
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.status-badge {
		padding: 4px 10px;
		background: var(--success-bg);
		border: 1px solid var(--success-border);
		border-radius: 12px;
		font-size: 12px;
		color: var(--success-color);
	}

	.status-badge.warning {
		background: #fef3c7;
		border-color: #f59e0b;
		color: #92400e;
	}

	.warning-banner {
		display: flex;
		align-items: center;
		padding: 10px 14px;
		background: #fef3c7;
		border: 1px solid #f59e0b;
		border-radius: 6px;
		color: #92400e;
		font-size: 13px;
		margin-bottom: 16px;
	}

	.image-select {
		padding: 6px 10px;
		border-radius: 6px;
		border: 1px solid var(--border-color);
		background: var(--button-secondary-bg);
		color: var(--text-primary);
		font-size: 13px;
		cursor: pointer;
	}

	.image-select:hover {
		background: var(--button-secondary-hover);
	}

	.btn-primary,
	.btn-secondary {
		padding: 6px 14px;
		border-radius: 6px;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
	}

	.btn-primary {
		border: none;
		background: var(--accent-color);
		color: white;
	}

	.btn-primary:hover {
		background: var(--accent-hover);
	}

	.btn-secondary {
		border: 1px solid var(--border-color);
		background: var(--button-secondary-bg);
		color: var(--text-primary);
	}

	.btn-secondary:hover {
		background: var(--button-secondary-hover);
	}

	.error-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 14px;
		background: var(--error-bg);
		border: 1px solid var(--error-border);
		border-radius: 6px;
		color: var(--error-color);
		font-size: 13px;
		margin-bottom: 16px;
	}

	.error-banner button {
		padding: 4px 10px;
		border-radius: 4px;
		border: none;
		background: var(--error-color);
		color: white;
		font-size: 12px;
		cursor: pointer;
	}

	.terminal-container {
		flex: 1;
		background: #1e1e1e;
		border-radius: 8px;
		overflow: hidden;
		min-height: 300px;
	}

	.terminal {
		height: 100%;
		padding: 8px;
	}

	:global(.terminal .xterm) {
		height: 100%;
	}

	:global(.terminal .xterm-viewport) {
		overflow-y: auto !important;
	}
</style>
