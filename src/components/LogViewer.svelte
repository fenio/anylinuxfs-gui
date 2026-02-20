<script lang="ts">
	import { logs, type LogLine } from '$lib/stores/logs';
	import { onMount, onDestroy } from 'svelte';

	// Virtualization settings
	const LINE_HEIGHT = 24; // px per line
	const BUFFER_SIZE = 10; // Extra lines to render above/below viewport

	let logContainer: HTMLDivElement | undefined = $state();
	let scrollTop = $state(0);
	let containerHeight = $state(0);

	// Computed virtualization values
	let visibleStart = $derived(Math.max(0, Math.floor(scrollTop / LINE_HEIGHT) - BUFFER_SIZE));
	let visibleCount = $derived(Math.ceil(containerHeight / LINE_HEIGHT) + BUFFER_SIZE * 2);
	let visibleEnd = $derived(Math.min($logs.lines.length, visibleStart + visibleCount));
	let visibleLines = $derived($logs.lines.slice(visibleStart, visibleEnd));
	let totalHeight = $derived($logs.lines.length * LINE_HEIGHT);
	let offsetY = $derived(visibleStart * LINE_HEIGHT);

	onMount(() => {
		logs.load();
		logs.startStreaming();
	});

	onDestroy(() => {
		logs.stopStreaming();
	});

	$effect(() => {
		if ($logs.following && logContainer && $logs.lines.length > 0) {
			// Use requestAnimationFrame to avoid layout thrashing
			requestAnimationFrame(() => {
				if (logContainer) {
					logContainer.scrollTop = logContainer.scrollHeight;
				}
			});
		}
	});

	function handleScroll() {
		if (!logContainer) return;
		scrollTop = logContainer.scrollTop;
		const { scrollHeight, clientHeight } = logContainer;
		const atBottom = scrollHeight - scrollTop - clientHeight < 50;
		if (atBottom !== $logs.following) {
			logs.setFollowing(atBottom);
		}
	}

	function handleResize() {
		if (logContainer) {
			containerHeight = logContainer.clientHeight;
		}
	}

	function scrollToBottom() {
		if (logContainer) {
			logContainer.scrollTop = logContainer.scrollHeight;
			logs.setFollowing(true);
		}
	}

	// Initialize container height
	$effect(() => {
		if (logContainer) {
			containerHeight = logContainer.clientHeight;
		}
	});
</script>

<svelte:window onresize={handleResize} />

<div class="log-viewer">
	<div class="header">
		<h2>Logs</h2>
		<div class="controls">
			<span class="line-count">{$logs.lines.length} lines</span>
			<label class="follow-toggle">
				<input
					type="checkbox"
					checked={$logs.following}
					onchange={(e) => logs.setFollowing((e.target as HTMLInputElement).checked)}
				/>
				Auto-scroll
			</label>
			<button class="btn-small" onclick={() => logs.clear()}>Clear</button>
			<button class="btn-small" onclick={() => logs.load()}>Reload</button>
		</div>
	</div>

	{#if $logs.error}
		<div class="error-banner" role="alert">
			<span>{$logs.error}</span>
		</div>
	{/if}

	<div
		class="log-content"
		bind:this={logContainer}
		onscroll={handleScroll}
	>
		{#if $logs.loading}
			<div class="loading">Loading logs...</div>
		{:else if $logs.lines.length === 0}
			<div class="empty">No log entries yet.</div>
		{:else}
			<!-- Virtual scroll container -->
			<div class="virtual-scroll" style="height: {totalHeight}px;">
				<div class="virtual-content" style="transform: translateY({offsetY}px);">
					{#each visibleLines as line, i (visibleStart + i)}
						<div
							class="log-line"
							class:error={line.isError}
							class:warn={line.isWarn}
						>
							<span class="line-number">{visibleStart + i + 1}</span>
							<span class="line-content">{line.text}</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</div>

	{#if !$logs.following && $logs.lines.length > 0}
		<button class="scroll-to-bottom" onclick={scrollToBottom}>
			Scroll to bottom
		</button>
	{/if}
</div>

<style>
	.log-viewer {
		display: flex;
		flex-direction: column;
		height: 100%;
		position: relative;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
		flex-shrink: 0;
	}

	.header h2 {
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.controls {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.line-count {
		font-size: 12px;
		color: var(--text-tertiary);
	}

	.follow-toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--text-secondary);
		cursor: pointer;
	}

	.follow-toggle input {
		cursor: pointer;
	}

	.btn-small {
		padding: 4px 10px;
		border-radius: 4px;
		border: 1px solid var(--border-color);
		background: var(--button-secondary-bg);
		color: var(--text-primary);
		font-size: 12px;
		cursor: pointer;
	}

	.btn-small:hover {
		background: var(--button-secondary-hover);
	}

	.error-banner {
		padding: 10px 14px;
		background: var(--error-bg);
		border: 1px solid var(--error-border);
		border-radius: 6px;
		color: var(--error-color);
		font-size: 13px;
		margin-bottom: 12px;
		flex-shrink: 0;
	}

	.log-content {
		flex: 1;
		overflow-y: auto;
		background: var(--log-bg);
		border: 1px solid var(--border-color);
		border-radius: 8px;
		font-family: 'SF Mono', 'Monaco', 'Menlo', monospace;
		font-size: 12px;
		line-height: 1.5;
	}

	.loading,
	.empty {
		padding: 24px;
		text-align: center;
		color: var(--log-text-secondary);
	}

	.virtual-scroll {
		position: relative;
	}

	.virtual-content {
		position: absolute;
		left: 0;
		right: 0;
		will-change: transform;
	}

	.log-line {
		display: flex;
		padding: 2px 12px;
		border-bottom: 1px solid var(--log-line-border);
		height: 24px;
		box-sizing: border-box;
	}

	.log-line:hover {
		background: var(--log-line-hover);
	}

	.log-line.error {
		background: var(--error-bg);
	}

	.log-line.warn {
		background: var(--warning-bg);
	}

	.line-number {
		width: 50px;
		flex-shrink: 0;
		color: var(--log-text-secondary);
		user-select: none;
	}

	.line-content {
		flex: 1;
		color: var(--log-text);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.scroll-to-bottom {
		position: absolute;
		bottom: 20px;
		left: 50%;
		transform: translateX(-50%);
		padding: 8px 16px;
		border-radius: 20px;
		border: none;
		background: var(--accent-color);
		color: white;
		font-size: 13px;
		cursor: pointer;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
	}

	.scroll-to-bottom:hover {
		background: var(--accent-hover);
	}
</style>
